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
        payload_hash: &PayloadHash,
    ) -> Result<RouteSubmissionResponse, Error> {
        let retry_result = self
            .idempotency_store
            .lookup(idempotency_key, payload_hash)
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
            user_id,
            created_at: Utc::now(),
        };

        match self.idempotency_store.store(&record).await {
            Ok(()) => {
                // TODO: Dispatch to route queue here when integrated.
                Ok(response)
            }
            Err(IdempotencyStoreError::DuplicateKey { .. }) => {
                self.handle_duplicate_key_race(&idempotency_key, &payload_hash)
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

        // Look up existing record for this key.
        let lookup_result = self
            .idempotency_store
            .lookup(&idempotency_key, &payload_hash)
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
    use crate::domain::ports::{FixtureIdempotencyStore, RouteSubmissionStatus};
    use crate::domain::{IdempotencyKey, UserId};
    use serde_json::json;

    fn make_service() -> RouteSubmissionServiceImpl<FixtureIdempotencyStore> {
        RouteSubmissionServiceImpl::new(Arc::new(FixtureIdempotencyStore))
    }

    #[tokio::test]
    async fn accepts_request_without_idempotency_key() {
        let service = make_service();
        let request = RouteSubmissionRequest {
            idempotency_key: None,
            user_id: UserId::random(),
            payload: json!({"origin": "A", "destination": "B"}),
        };

        let response = service
            .submit(request)
            .await
            .expect("submission should succeed");
        assert_eq!(response.status, RouteSubmissionStatus::Accepted);
    }

    #[tokio::test]
    async fn accepts_request_with_new_idempotency_key() {
        let service = make_service();
        let request = RouteSubmissionRequest {
            idempotency_key: Some(IdempotencyKey::random()),
            user_id: UserId::random(),
            payload: json!({"origin": "A", "destination": "B"}),
        };

        // FixtureIdempotencyStore always returns NotFound, so new keys are accepted.
        let response = service
            .submit(request)
            .await
            .expect("submission should succeed");
        assert_eq!(response.status, RouteSubmissionStatus::Accepted);
    }
}
