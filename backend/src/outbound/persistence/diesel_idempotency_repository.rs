//! PostgreSQL-backed `IdempotencyRepository` implementation using Diesel ORM.
//!
//! This adapter implements the domain's `IdempotencyRepository` port, providing
//! durable storage for idempotency records. All database operations are async
//! via `diesel-async`.
//!
//! # TTL Enforcement
//!
//! Records are not filtered by TTL during lookups. Instead, the `cleanup_expired`
//! method should be called periodically (e.g., on startup or via a scheduled job)
//! to remove stale records.
//!
//! # Mutation Type Scoping
//!
//! Idempotency records are scoped by mutation type, allowing the same UUID to be
//! used as an idempotency key across different operation types (routes, notes,
//! progress, preferences, bundles) without collision.

use std::str::FromStr;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use tracing::debug;

use crate::domain::ports::{IdempotencyRepository, IdempotencyRepositoryError};
use crate::domain::{
    IdempotencyKey, IdempotencyLookupResult, IdempotencyRecord, MutationType, PayloadHash, UserId,
};

use super::models::{IdempotencyKeyRow, NewIdempotencyKeyRow};
use super::pool::{DbPool, PoolError};
use super::schema::idempotency_keys;

/// Diesel-backed implementation of the `IdempotencyRepository` port.
///
/// Provides PostgreSQL persistence for idempotency records, enabling safe
/// request retries by detecting duplicate requests and replaying previous
/// responses. Records are scoped by user ID and mutation type.
#[derive(Clone)]
pub struct DieselIdempotencyRepository {
    pool: DbPool,
}

impl DieselIdempotencyRepository {
    /// Create a new repository with the given connection pool.
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

/// Map pool errors to domain idempotency repository errors.
fn map_pool_error(error: PoolError) -> IdempotencyRepositoryError {
    match error {
        PoolError::Checkout { message } | PoolError::Build { message } => {
            IdempotencyRepositoryError::connection(message)
        }
    }
}

/// Map Diesel errors to domain idempotency repository errors.
fn map_diesel_error(error: diesel::result::Error) -> IdempotencyRepositoryError {
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
        DieselError::NotFound => IdempotencyRepositoryError::query("record not found"),
        DieselError::QueryBuilderError(_) => {
            IdempotencyRepositoryError::query("database query error")
        }
        DieselError::DatabaseError(kind, _) => match kind {
            DatabaseErrorKind::UniqueViolation => {
                IdempotencyRepositoryError::duplicate_key("concurrent insert detected")
            }
            DatabaseErrorKind::ClosedConnection => {
                IdempotencyRepositoryError::connection("database connection error")
            }
            _ => IdempotencyRepositoryError::query("database error"),
        },
        _ => IdempotencyRepositoryError::query("database error"),
    }
}

/// Convert a database row to a domain IdempotencyRecord.
fn row_to_record(row: IdempotencyKeyRow) -> Result<IdempotencyRecord, IdempotencyRepositoryError> {
    let key = IdempotencyKey::from_uuid(row.key);
    let payload_hash = PayloadHash::try_from_bytes(&row.payload_hash).map_err(|err| {
        IdempotencyRepositoryError::query(format!("corrupted payload hash in database: {err}"))
    })?;
    let user_id = UserId::from_uuid(row.user_id);
    let mutation_type = MutationType::from_str(&row.mutation_type).map_err(|err| {
        IdempotencyRepositoryError::query(format!("invalid mutation type in database: {err}"))
    })?;

    Ok(IdempotencyRecord {
        key,
        mutation_type,
        payload_hash,
        response_snapshot: row.response_snapshot,
        user_id,
        created_at: row.created_at,
    })
}

#[async_trait]
impl IdempotencyRepository for DieselIdempotencyRepository {
    async fn lookup(
        &self,
        key: &IdempotencyKey,
        user_id: &UserId,
        mutation_type: MutationType,
        payload_hash: &PayloadHash,
    ) -> Result<IdempotencyLookupResult, IdempotencyRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let result: Option<IdempotencyKeyRow> = idempotency_keys::table
            .filter(
                idempotency_keys::key
                    .eq(key.as_uuid())
                    .and(idempotency_keys::user_id.eq(user_id.as_uuid()))
                    .and(idempotency_keys::mutation_type.eq(mutation_type.as_str())),
            )
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

    async fn store(&self, record: &IdempotencyRecord) -> Result<(), IdempotencyRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let new_record = NewIdempotencyKeyRow {
            key: *record.key.as_uuid(),
            mutation_type: record.mutation_type.as_str(),
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

    async fn cleanup_expired(&self, ttl: Duration) -> Result<u64, IdempotencyRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let cutoff = Utc::now()
            - chrono::Duration::from_std(ttl).map_err(|err| {
                IdempotencyRepositoryError::query(format!("invalid TTL duration: {err}"))
            })?;

        let deleted = diesel::delete(idempotency_keys::table)
            .filter(idempotency_keys::created_at.lt(cutoff))
            .execute(&mut conn)
            .await
            .map_err(map_diesel_error)?;

        debug!(deleted, cutoff = %cutoff, "cleaned up expired idempotency records");
        #[expect(clippy::expect_used, reason = "usize row count always fits in u64")]
        Ok(u64::try_from(deleted).expect("row count fits in u64"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn pool_error_maps_to_connection_error() {
        let pool_err = PoolError::checkout("connection refused");
        let repo_err = map_pool_error(pool_err);

        assert!(matches!(
            repo_err,
            IdempotencyRepositoryError::Connection { .. }
        ));
        assert!(repo_err.to_string().contains("connection refused"));
    }

    #[rstest]
    fn diesel_error_maps_to_query_error() {
        let diesel_err = diesel::result::Error::NotFound;
        let repo_err = map_diesel_error(diesel_err);

        assert!(matches!(repo_err, IdempotencyRepositoryError::Query { .. }));
        assert!(repo_err.to_string().contains("record not found"));
    }

    #[rstest]
    fn unique_violation_maps_to_duplicate_key() {
        use diesel::result::{DatabaseErrorKind, Error as DieselError};

        let diesel_err = DieselError::DatabaseError(
            DatabaseErrorKind::UniqueViolation,
            Box::new("duplicate key".to_string()),
        );
        let repo_err = map_diesel_error(diesel_err);

        assert!(
            matches!(repo_err, IdempotencyRepositoryError::DuplicateKey { .. }),
            "expected DuplicateKey error, got {repo_err:?}"
        );
        assert!(
            repo_err.to_string().contains("concurrent insert"),
            "expected 'concurrent insert' in message, got: {}",
            repo_err
        );
    }
}
