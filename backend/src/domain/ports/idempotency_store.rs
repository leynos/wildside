//! Port abstraction for idempotency key persistence.
//!
//! The [`IdempotencyStore`] trait defines the contract for storing and
//! retrieving idempotency records. Adapters implement this trait to provide
//! durable storage (e.g., PostgreSQL) that survives server restarts.

use std::time::Duration;

use async_trait::async_trait;

use crate::domain::{
    IdempotencyKey, IdempotencyLookupResult, IdempotencyRecord, PayloadHash, UserId,
};

use super::define_port_error;

define_port_error! {
    /// Errors raised by idempotency store adapters.
    pub enum IdempotencyStoreError {
        /// Store connection could not be established.
        Connection { message: String } => "idempotency store connection failed: {message}",
        /// Query or mutation failed during execution.
        Query { message: String } => "idempotency store query failed: {message}",
        /// Response serialization or deserialization failed.
        Serialization { message: String } => "idempotency store serialization failed: {message}",
        /// A record with this key already exists (concurrent insert race).
        DuplicateKey { message: String } => "idempotency key already exists: {message}",
    }
}

/// Port for idempotency key storage and retrieval.
///
/// Implementations provide durable storage for idempotency records, enabling
/// safe request retries by detecting duplicate requests and replaying previous
/// responses.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait IdempotencyStore: Send + Sync {
    /// Look up an idempotency key for a specific user and compare against the
    /// provided payload hash.
    ///
    /// The lookup is scoped to the given user to prevent cross-user key reuse.
    ///
    /// Returns:
    /// - [`IdempotencyLookupResult::NotFound`] if no record exists for the key.
    /// - [`IdempotencyLookupResult::MatchingPayload`] if a record exists and the
    ///   payload hash matches.
    /// - [`IdempotencyLookupResult::ConflictingPayload`] if a record exists but
    ///   the payload hash differs.
    async fn lookup(
        &self,
        key: &IdempotencyKey,
        user_id: &UserId,
        payload_hash: &PayloadHash,
    ) -> Result<IdempotencyLookupResult, IdempotencyStoreError>;

    /// Store an idempotency record.
    ///
    /// If a record already exists for the key, the behaviour is
    /// implementation-defined (typically a no-op or conflict error).
    async fn store(&self, record: &IdempotencyRecord) -> Result<(), IdempotencyStoreError>;

    /// Remove records older than the specified TTL.
    ///
    /// Returns the number of records deleted.
    async fn cleanup_expired(&self, ttl: Duration) -> Result<u64, IdempotencyStoreError>;
}

/// Fixture implementation for testing without a real database.
///
/// This implementation always returns `NotFound` and discards stored records.
/// Use it in unit tests where idempotency behaviour is not under test.
#[derive(Debug, Default)]
pub struct FixtureIdempotencyStore;

#[async_trait]
impl IdempotencyStore for FixtureIdempotencyStore {
    async fn lookup(
        &self,
        _key: &IdempotencyKey,
        _user_id: &UserId,
        _payload_hash: &PayloadHash,
    ) -> Result<IdempotencyLookupResult, IdempotencyStoreError> {
        Ok(IdempotencyLookupResult::NotFound)
    }

    async fn store(&self, _record: &IdempotencyRecord) -> Result<(), IdempotencyStoreError> {
        Ok(())
    }

    async fn cleanup_expired(&self, _ttl: Duration) -> Result<u64, IdempotencyStoreError> {
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{canonicalize_and_hash, UserId};
    use chrono::Utc;
    use serde_json::json;

    #[tokio::test]
    async fn fixture_store_lookup_returns_not_found() {
        let store = FixtureIdempotencyStore;
        let key = IdempotencyKey::random();
        let user_id = UserId::random();
        let hash = canonicalize_and_hash(&json!({"test": true}));

        let result = store
            .lookup(&key, &user_id, &hash)
            .await
            .expect("fixture lookup should succeed");
        assert!(matches!(result, IdempotencyLookupResult::NotFound));
    }

    #[tokio::test]
    async fn fixture_store_accepts_store_operations() {
        let store = FixtureIdempotencyStore;
        let record = IdempotencyRecord {
            key: IdempotencyKey::random(),
            payload_hash: canonicalize_and_hash(&json!({"test": true})),
            response_snapshot: json!({"request_id": "123"}),
            user_id: UserId::random(),
            created_at: Utc::now(),
        };

        store
            .store(&record)
            .await
            .expect("fixture store should accept records");
    }

    #[tokio::test]
    async fn fixture_store_cleanup_returns_zero() {
        let store = FixtureIdempotencyStore;
        let deleted = store
            .cleanup_expired(Duration::from_secs(3600))
            .await
            .expect("fixture cleanup should succeed");
        assert_eq!(deleted, 0);
    }
}
