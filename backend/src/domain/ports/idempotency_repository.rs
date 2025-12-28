//! Port abstraction for idempotency key persistence.
//!
//! The [`IdempotencyRepository`] trait defines the contract for storing and
//! retrieving idempotency records. Adapters implement this trait to provide
//! durable storage (e.g., PostgreSQL) that survives server restarts.
//!
//! The repository supports multiple mutation types, allowing keys to be scoped
//! per operation kind (routes, notes, progress, preferences, bundles).

use std::time::Duration;

use async_trait::async_trait;

use crate::domain::{
    IdempotencyKey, IdempotencyLookupResult, IdempotencyRecord, MutationType, PayloadHash, UserId,
};

use super::define_port_error;

define_port_error! {
    /// Errors raised by idempotency repository adapters.
    pub enum IdempotencyRepositoryError {
        /// Repository connection could not be established.
        Connection { message: String } => "idempotency repository connection failed: {message}",
        /// Query or mutation failed during execution.
        Query { message: String } => "idempotency repository query failed: {message}",
        /// Response serialization or deserialization failed.
        Serialization { message: String } => "idempotency repository serialization failed: {message}",
        /// A record with this key already exists (concurrent insert race).
        DuplicateKey { message: String } => "idempotency key already exists: {message}",
    }
}

/// Port for idempotency record storage and retrieval.
///
/// Implementations provide durable storage for idempotency records, enabling
/// safe request retries by detecting duplicate requests and replaying previous
/// responses. The repository supports multiple mutation types, allowing keys
/// to be scoped per operation kind.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
#[allow(clippy::too_many_arguments)]
pub trait IdempotencyRepository: Send + Sync {
    /// Look up an idempotency key for a specific user and mutation type.
    ///
    /// The lookup is scoped to the given user and mutation type to prevent
    /// cross-user or cross-operation key reuse.
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
        mutation_type: MutationType,
        payload_hash: &PayloadHash,
    ) -> Result<IdempotencyLookupResult, IdempotencyRepositoryError>;

    /// Store an idempotency record.
    ///
    /// If a record already exists for the key, the behaviour is
    /// implementation-defined (typically a no-op or conflict error).
    async fn store(&self, record: &IdempotencyRecord) -> Result<(), IdempotencyRepositoryError>;

    /// Remove records older than the specified TTL.
    ///
    /// Returns the number of records deleted.
    async fn cleanup_expired(&self, ttl: Duration) -> Result<u64, IdempotencyRepositoryError>;
}

/// Fixture implementation for testing without a real database.
///
/// This implementation always returns `NotFound` and discards stored records.
/// Use it in unit tests where idempotency behaviour is not under test.
#[derive(Debug, Default)]
pub struct FixtureIdempotencyRepository;

#[async_trait]
impl IdempotencyRepository for FixtureIdempotencyRepository {
    async fn lookup(
        &self,
        _key: &IdempotencyKey,
        _user_id: &UserId,
        _mutation_type: MutationType,
        _payload_hash: &PayloadHash,
    ) -> Result<IdempotencyLookupResult, IdempotencyRepositoryError> {
        Ok(IdempotencyLookupResult::NotFound)
    }

    async fn store(&self, _record: &IdempotencyRecord) -> Result<(), IdempotencyRepositoryError> {
        Ok(())
    }

    async fn cleanup_expired(&self, _ttl: Duration) -> Result<u64, IdempotencyRepositoryError> {
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{MutationType, UserId, canonicalize_and_hash};
    use chrono::Utc;
    use serde_json::json;

    #[tokio::test]
    async fn fixture_repository_lookup_returns_not_found() {
        let repo = FixtureIdempotencyRepository;
        let key = IdempotencyKey::random();
        let user_id = UserId::random();
        let hash = canonicalize_and_hash(&json!({"test": true}));

        let result = repo
            .lookup(&key, &user_id, MutationType::Routes, &hash)
            .await
            .expect("fixture lookup should succeed");
        assert!(matches!(result, IdempotencyLookupResult::NotFound));
    }

    #[tokio::test]
    async fn fixture_repository_accepts_store_operations() {
        let repo = FixtureIdempotencyRepository;
        let record = IdempotencyRecord {
            key: IdempotencyKey::random(),
            mutation_type: MutationType::Routes,
            payload_hash: canonicalize_and_hash(&json!({"test": true})),
            response_snapshot: json!({"request_id": "123"}),
            user_id: UserId::random(),
            created_at: Utc::now(),
        };

        repo.store(&record)
            .await
            .expect("fixture store should accept records");
    }

    #[tokio::test]
    async fn fixture_repository_cleanup_returns_zero() {
        let repo = FixtureIdempotencyRepository;
        let deleted = repo
            .cleanup_expired(Duration::from_secs(3600))
            .await
            .expect("fixture cleanup should succeed");
        assert_eq!(deleted, 0);
    }

    #[tokio::test]
    async fn fixture_repository_lookup_with_different_mutation_types() {
        let repo = FixtureIdempotencyRepository;
        let key = IdempotencyKey::random();
        let user_id = UserId::random();
        let hash = canonicalize_and_hash(&json!({"test": true}));

        // All mutation types should return NotFound from the fixture
        for mutation_type in [
            MutationType::Routes,
            MutationType::Notes,
            MutationType::Progress,
            MutationType::Preferences,
            MutationType::Bundles,
        ] {
            let result = repo
                .lookup(&key, &user_id, mutation_type, &hash)
                .await
                .expect("fixture lookup should succeed");
            assert!(
                matches!(result, IdempotencyLookupResult::NotFound),
                "expected NotFound for {mutation_type:?}"
            );
        }
    }
}
