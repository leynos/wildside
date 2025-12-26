//! Route submission orchestration with idempotency support.
//!
//! This module provides the concrete implementation of [`RouteSubmissionService`]
//! that coordinates idempotency checking, job dispatch, and response storage.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;

use super::ports::{
    IdempotencyStore, IdempotencyStoreError, RouteSubmissionRequest, RouteSubmissionResponse,
    RouteSubmissionService,
};
use super::{
    canonicalize_and_hash, Error, IdempotencyKey, IdempotencyLookupResult, IdempotencyRecord,
    PayloadHash, UserId,
};

/// Concrete implementation of [`RouteSubmissionService`].
///
/// Orchestrates route submission with idempotency support by:
/// 1. Looking up idempotency keys in the store.
/// 2. Comparing payload hashes to detect conflicts.
/// 3. Generating request IDs for new submissions.
/// 4. Storing responses for future replay.
pub struct RouteSubmissionServiceImpl<S> {
    idempotency_store: Arc<S>,
}

impl<S> RouteSubmissionServiceImpl<S>
where
    S: IdempotencyStore,
{
    /// Create a new service with the given idempotency store.
    pub fn new(idempotency_store: Arc<S>) -> Self {
        Self { idempotency_store }
    }

    /// Deserialise a stored response snapshot.
    fn deserialize_stored_response(
        snapshot: serde_json::Value,
    ) -> Result<RouteSubmissionResponse, Error> {
        serde_json::from_value(snapshot)
            .map_err(|err| Error::internal(format!("failed to deserialize stored response: {err}")))
    }

    /// Handle a duplicate key race by retrying lookup.
    async fn handle_duplicate_key_race(
        &self,
        idempotency_key: &IdempotencyKey,
        user_id: &UserId,
        payload_hash: &PayloadHash,
    ) -> Result<RouteSubmissionResponse, Error> {
        let retry_result = self
            .idempotency_store
            .lookup(idempotency_key, user_id, payload_hash)
            .await
            .map_err(map_store_error)?;

        match retry_result {
            IdempotencyLookupResult::MatchingPayload(existing) => {
                let stored_response =
                    Self::deserialize_stored_response(existing.response_snapshot)?;
                Ok(RouteSubmissionResponse::replayed(
                    stored_response.request_id,
                ))
            }
            IdempotencyLookupResult::ConflictingPayload(_) => Err(Error::conflict(
                "idempotency key already used with different payload",
            )),
            IdempotencyLookupResult::NotFound => Err(Error::internal(
                "idempotency record disappeared during race resolution",
            )),
        }
    }

    /// Handle a new idempotent request by storing and dispatching.
    async fn handle_new_request(
        &self,
        idempotency_key: IdempotencyKey,
        payload_hash: PayloadHash,
        user_id: UserId,
    ) -> Result<RouteSubmissionResponse, Error> {
        let request_id = Uuid::new_v4();
        let response = RouteSubmissionResponse::accepted(request_id);
        let response_snapshot = serde_json::to_value(&response)
            .map_err(|err| Error::internal(format!("failed to serialize response: {err}")))?;

        let record = IdempotencyRecord {
            key: idempotency_key.clone(),
            payload_hash: payload_hash.clone(),
            response_snapshot,
            user_id: user_id.clone(),
            created_at: Utc::now(),
        };

        match self.idempotency_store.store(&record).await {
            Ok(()) => {
                // TODO: Dispatch to route queue here when integrated.
                Ok(response)
            }
            Err(IdempotencyStoreError::DuplicateKey { .. }) => {
                self.handle_duplicate_key_race(&idempotency_key, &user_id, &payload_hash)
                    .await
            }
            Err(err) => Err(map_store_error(err)),
        }
    }
}

/// Map idempotency store errors to domain errors.
fn map_store_error(error: IdempotencyStoreError) -> Error {
    match error {
        IdempotencyStoreError::Connection { message } => {
            Error::service_unavailable(format!("idempotency store unavailable: {message}"))
        }
        IdempotencyStoreError::Query { message } => {
            Error::internal(format!("idempotency store error: {message}"))
        }
        IdempotencyStoreError::Serialization { message } => {
            Error::internal(format!("response serialization failed: {message}"))
        }
        IdempotencyStoreError::DuplicateKey { message } => {
            // This shouldn't reach here if race handling works correctly.
            Error::internal(format!("unexpected duplicate key: {message}"))
        }
    }
}

#[async_trait]
impl<S> RouteSubmissionService for RouteSubmissionServiceImpl<S>
where
    S: IdempotencyStore,
{
    async fn submit(
        &self,
        request: RouteSubmissionRequest,
    ) -> Result<RouteSubmissionResponse, Error> {
        let payload_hash = canonicalize_and_hash(&request.payload);

        // If no idempotency key, proceed without tracking.
        let Some(idempotency_key) = request.idempotency_key else {
            let request_id = Uuid::new_v4();
            // TODO: Dispatch to route queue here when integrated.
            return Ok(RouteSubmissionResponse::accepted(request_id));
        };

        // Look up existing record for this key (scoped to user).
        let lookup_result = self
            .idempotency_store
            .lookup(&idempotency_key, &request.user_id, &payload_hash)
            .await
            .map_err(map_store_error)?;

        match lookup_result {
            IdempotencyLookupResult::NotFound => {
                self.handle_new_request(idempotency_key, payload_hash, request.user_id)
                    .await
            }
            IdempotencyLookupResult::MatchingPayload(record) => {
                let stored_response = Self::deserialize_stored_response(record.response_snapshot)?;
                Ok(RouteSubmissionResponse::replayed(
                    stored_response.request_id,
                ))
            }
            IdempotencyLookupResult::ConflictingPayload(_) => Err(Error::conflict(
                "idempotency key already used with different payload",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ports::{
        FixtureIdempotencyStore, MockIdempotencyStore, RouteSubmissionStatus,
    };
    use crate::domain::{canonicalize_and_hash, IdempotencyKey, UserId};
    use mockall::predicate::*;
    use serde_json::json;

    /// Helper to build a RouteSubmissionRequest for tests.
    fn build_request(
        idempotency_key: Option<IdempotencyKey>,
        user_id: UserId,
        payload: serde_json::Value,
    ) -> RouteSubmissionRequest {
        RouteSubmissionRequest {
            idempotency_key,
            user_id,
            payload,
        }
    }

    /// Helper to build an IdempotencyRecord fixture.
    fn build_record(
        key: IdempotencyKey,
        payload_hash: PayloadHash,
        request_id: Uuid,
        user_id: UserId,
    ) -> IdempotencyRecord {
        let response = RouteSubmissionResponse::accepted(request_id);
        let response_snapshot =
            serde_json::to_value(&response).expect("serialization should succeed");

        IdempotencyRecord {
            key,
            payload_hash,
            response_snapshot,
            user_id,
            created_at: Utc::now(),
        }
    }

    /// Default test payload.
    fn default_payload() -> serde_json::Value {
        json!({"origin": "A", "destination": "B"})
    }

    /// Alternative test payload (different from default).
    fn alternative_payload() -> serde_json::Value {
        json!({"origin": "X", "destination": "Y"})
    }

    fn make_service() -> RouteSubmissionServiceImpl<FixtureIdempotencyStore> {
        RouteSubmissionServiceImpl::new(Arc::new(FixtureIdempotencyStore))
    }

    #[tokio::test]
    async fn accepts_request_without_idempotency_key() {
        let service = make_service();
        let request = build_request(None, UserId::random(), default_payload());

        let response = service
            .submit(request)
            .await
            .expect("submission should succeed");
        assert_eq!(response.status, RouteSubmissionStatus::Accepted);
    }

    #[tokio::test]
    async fn accepts_request_with_new_idempotency_key() {
        let service = make_service();
        let request = build_request(
            Some(IdempotencyKey::random()),
            UserId::random(),
            default_payload(),
        );

        // FixtureIdempotencyStore always returns NotFound, so new keys are accepted.
        let response = service
            .submit(request)
            .await
            .expect("submission should succeed");
        assert_eq!(response.status, RouteSubmissionStatus::Accepted);
    }

    #[tokio::test]
    async fn replays_response_for_matching_payload() {
        let idempotency_key = IdempotencyKey::random();
        let user_id = UserId::random();
        let payload = default_payload();
        let payload_hash = canonicalize_and_hash(&payload);
        let original_request_id = Uuid::new_v4();

        let existing_record = build_record(
            idempotency_key.clone(),
            payload_hash,
            original_request_id,
            user_id.clone(),
        );

        let mut mock_store = MockIdempotencyStore::new();
        mock_store
            .expect_lookup()
            .with(eq(idempotency_key.clone()), eq(user_id.clone()), always())
            .times(1)
            .returning(move |_, _, _| {
                Ok(IdempotencyLookupResult::MatchingPayload(
                    existing_record.clone(),
                ))
            });

        let service = RouteSubmissionServiceImpl::new(Arc::new(mock_store));
        let request = build_request(Some(idempotency_key), user_id, payload);

        let response = service
            .submit(request)
            .await
            .expect("submission should succeed");

        assert_eq!(response.status, RouteSubmissionStatus::Replayed);
        assert_eq!(response.request_id, original_request_id);
    }

    #[tokio::test]
    async fn returns_conflict_for_different_payload() {
        let idempotency_key = IdempotencyKey::random();
        let user_id = UserId::random();
        let original_hash = canonicalize_and_hash(&default_payload());

        let existing_record = build_record(
            idempotency_key.clone(),
            original_hash,
            Uuid::new_v4(),
            user_id.clone(),
        );

        let mut mock_store = MockIdempotencyStore::new();
        mock_store
            .expect_lookup()
            .with(eq(idempotency_key.clone()), eq(user_id.clone()), always())
            .times(1)
            .returning(move |_, _, _| {
                Ok(IdempotencyLookupResult::ConflictingPayload(
                    existing_record.clone(),
                ))
            });

        let service = RouteSubmissionServiceImpl::new(Arc::new(mock_store));
        let request = build_request(Some(idempotency_key), user_id, alternative_payload());

        let error = service
            .submit(request)
            .await
            .expect_err("submission should fail with conflict");

        assert_eq!(error.code(), crate::domain::ErrorCode::Conflict);
        assert!(error
            .message()
            .contains("idempotency key already used with different payload"));
    }

    #[tokio::test]
    async fn handles_concurrent_insert_race_with_matching_payload() {
        let idempotency_key = IdempotencyKey::random();
        let user_id = UserId::random();
        let payload = default_payload();
        let payload_hash = canonicalize_and_hash(&payload);
        let original_request_id = Uuid::new_v4();

        let existing_record = build_record(
            idempotency_key.clone(),
            payload_hash,
            original_request_id,
            user_id.clone(),
        );

        let mut mock_store = MockIdempotencyStore::new();

        // First lookup returns NotFound (simulating a race where another request
        // inserted between our lookup and store).
        mock_store
            .expect_lookup()
            .with(eq(idempotency_key.clone()), eq(user_id.clone()), always())
            .times(1)
            .returning(|_, _, _| Ok(IdempotencyLookupResult::NotFound));

        // Store fails with DuplicateKey (the other request won the race).
        mock_store
            .expect_store()
            .times(1)
            .returning(|_| Err(IdempotencyStoreError::duplicate_key("concurrent insert")));

        // Retry lookup after race returns MatchingPayload.
        let record_for_retry = existing_record.clone();
        mock_store
            .expect_lookup()
            .with(eq(idempotency_key.clone()), eq(user_id.clone()), always())
            .times(1)
            .returning(move |_, _, _| {
                Ok(IdempotencyLookupResult::MatchingPayload(
                    record_for_retry.clone(),
                ))
            });

        let service = RouteSubmissionServiceImpl::new(Arc::new(mock_store));
        let request = build_request(Some(idempotency_key), user_id, payload);

        let response = service
            .submit(request)
            .await
            .expect("submission should succeed after race resolution");

        assert_eq!(response.status, RouteSubmissionStatus::Replayed);
        assert_eq!(response.request_id, original_request_id);
    }

    #[tokio::test]
    async fn handles_concurrent_insert_race_with_conflicting_payload() {
        let idempotency_key = IdempotencyKey::random();
        let user_id = UserId::random();
        let our_payload = default_payload();
        let their_hash = canonicalize_and_hash(&alternative_payload());

        let their_record = build_record(
            idempotency_key.clone(),
            their_hash,
            Uuid::new_v4(),
            user_id.clone(),
        );

        let mut mock_store = MockIdempotencyStore::new();

        // First lookup returns NotFound.
        mock_store
            .expect_lookup()
            .with(eq(idempotency_key.clone()), eq(user_id.clone()), always())
            .times(1)
            .returning(|_, _, _| Ok(IdempotencyLookupResult::NotFound));

        // Store fails with DuplicateKey.
        mock_store
            .expect_store()
            .times(1)
            .returning(|_| Err(IdempotencyStoreError::duplicate_key("concurrent insert")));

        // Retry lookup after race returns ConflictingPayload.
        let record_for_retry = their_record.clone();
        mock_store
            .expect_lookup()
            .with(eq(idempotency_key.clone()), eq(user_id.clone()), always())
            .times(1)
            .returning(move |_, _, _| {
                Ok(IdempotencyLookupResult::ConflictingPayload(
                    record_for_retry.clone(),
                ))
            });

        let service = RouteSubmissionServiceImpl::new(Arc::new(mock_store));
        let request = build_request(Some(idempotency_key), user_id, our_payload);

        let error = service
            .submit(request)
            .await
            .expect_err("submission should fail with conflict");

        assert_eq!(error.code(), crate::domain::ErrorCode::Conflict);
    }
}
