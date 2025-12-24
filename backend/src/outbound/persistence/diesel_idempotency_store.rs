//! PostgreSQL-backed `IdempotencyStore` implementation using Diesel ORM.
//!
//! This adapter implements the domain's `IdempotencyStore` port, providing
//! durable storage for idempotency records. All database operations are async
//! via `diesel-async`.
//!
//! # TTL Enforcement
//!
//! Records are not filtered by TTL during lookups. Instead, the `cleanup_expired`
//! method should be called periodically (e.g., on startup or via a scheduled job)
//! to remove stale records.

use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use tracing::debug;

use crate::domain::ports::{IdempotencyStore, IdempotencyStoreError};
use crate::domain::{
    IdempotencyKey, IdempotencyLookupResult, IdempotencyRecord, PayloadHash, UserId,
};

use super::models::{IdempotencyKeyRow, NewIdempotencyKeyRow};
use super::pool::{DbPool, PoolError};
use super::schema::idempotency_keys;

/// Diesel-backed implementation of the `IdempotencyStore` port.
///
/// Provides PostgreSQL persistence for idempotency records, enabling safe
/// request retries by detecting duplicate requests and replaying previous
/// responses.
#[derive(Clone)]
pub struct DieselIdempotencyStore {
    pool: DbPool,
}

impl DieselIdempotencyStore {
    /// Create a new store with the given connection pool.
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

/// Map pool errors to domain idempotency store errors.
fn map_pool_error(error: PoolError) -> IdempotencyStoreError {
    match error {
        PoolError::Checkout { message } | PoolError::Build { message } => {
            IdempotencyStoreError::connection(message)
        }
    }
}

/// Map Diesel errors to domain idempotency store errors.
fn map_diesel_error(error: diesel::result::Error) -> IdempotencyStoreError {
    use diesel::result::{DatabaseErrorKind, Error as DieselError};

    match &error {
        DieselError::DatabaseError(kind, info) => {
            debug!(?kind, message = info.message(), "diesel operation failed");
        }
        _ => debug!(
            error_type = %std::any::type_name_of_val(&error),
            "diesel operation failed"
        ),
    }

    match error {
        DieselError::NotFound => IdempotencyStoreError::query("record not found"),
        DieselError::QueryBuilderError(_) => IdempotencyStoreError::query("database query error"),
        DieselError::DatabaseError(kind, _) => match kind {
            DatabaseErrorKind::UniqueViolation => {
                IdempotencyStoreError::query("duplicate idempotency key")
            }
            DatabaseErrorKind::ClosedConnection => {
                IdempotencyStoreError::connection("database connection error")
            }
            _ => IdempotencyStoreError::query("database error"),
        },
        _ => IdempotencyStoreError::query("database error"),
    }
}

/// Convert a database row to a domain IdempotencyRecord.
fn row_to_record(row: IdempotencyKeyRow) -> Result<IdempotencyRecord, IdempotencyStoreError> {
    let key = IdempotencyKey::from_uuid(row.key);
    let payload_hash = PayloadHash::try_from_bytes(&row.payload_hash).map_err(|err| {
        IdempotencyStoreError::query(format!("corrupted payload hash in database: {err}"))
    })?;
    let user_id = UserId::from_uuid(row.user_id);

    Ok(IdempotencyRecord::new(
        key,
        payload_hash,
        row.response_snapshot,
        user_id,
        row.created_at,
    ))
}

#[async_trait]
impl IdempotencyStore for DieselIdempotencyStore {
    async fn lookup(
        &self,
        key: &IdempotencyKey,
        payload_hash: &PayloadHash,
    ) -> Result<IdempotencyLookupResult, IdempotencyStoreError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let result: Option<IdempotencyKeyRow> = idempotency_keys::table
            .filter(idempotency_keys::key.eq(key.as_uuid()))
            .select(IdempotencyKeyRow::as_select())
            .first(&mut conn)
            .await
            .optional()
            .map_err(map_diesel_error)?;

        match result {
            None => Ok(IdempotencyLookupResult::NotFound),
            Some(row) => {
                let record = row_to_record(row)?;
                if record.payload_hash == *payload_hash {
                    Ok(IdempotencyLookupResult::MatchingPayload(record))
                } else {
                    Ok(IdempotencyLookupResult::ConflictingPayload(record))
                }
            }
        }
    }

    async fn store(&self, record: &IdempotencyRecord) -> Result<(), IdempotencyStoreError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let new_record = NewIdempotencyKeyRow {
            key: *record.key.as_uuid(),
            payload_hash: record.payload_hash.as_bytes(),
            response_snapshot: &record.response_snapshot,
            user_id: *record.user_id.as_uuid(),
        };

        diesel::insert_into(idempotency_keys::table)
            .values(&new_record)
            .execute(&mut conn)
            .await
            .map(|_| ())
            .map_err(map_diesel_error)
    }

    async fn cleanup_expired(&self, ttl: Duration) -> Result<u64, IdempotencyStoreError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let cutoff = Utc::now()
            - chrono::Duration::from_std(ttl).map_err(|err| {
                IdempotencyStoreError::query(format!("invalid TTL duration: {err}"))
            })?;

        let deleted = diesel::delete(idempotency_keys::table)
            .filter(idempotency_keys::created_at.lt(cutoff))
            .execute(&mut conn)
            .await
            .map_err(map_diesel_error)?;

        debug!(deleted, cutoff = %cutoff, "cleaned up expired idempotency records");
        Ok(deleted as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn pool_error_maps_to_connection_error() {
        let pool_err = PoolError::checkout("connection refused");
        let store_err = map_pool_error(pool_err);

        assert!(matches!(
            store_err,
            IdempotencyStoreError::Connection { .. }
        ));
        assert!(store_err.to_string().contains("connection refused"));
    }

    #[rstest]
    fn diesel_error_maps_to_query_error() {
        let diesel_err = diesel::result::Error::NotFound;
        let store_err = map_diesel_error(diesel_err);

        assert!(matches!(store_err, IdempotencyStoreError::Query { .. }));
        assert!(store_err.to_string().contains("record not found"));
    }

    #[rstest]
    fn unique_violation_maps_to_duplicate_key() {
        use diesel::result::{DatabaseErrorKind, Error as DieselError};

        let diesel_err = DieselError::DatabaseError(
            DatabaseErrorKind::UniqueViolation,
            Box::new("duplicate key".to_string()),
        );
        let store_err = map_diesel_error(diesel_err);

        assert!(store_err.to_string().contains("duplicate idempotency key"));
    }
}
