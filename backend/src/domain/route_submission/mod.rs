//! Route submission orchestration with idempotency support.
//!
//! This module provides the concrete implementation of [`RouteSubmissionService`]
//! that coordinates idempotency checking, job dispatch, and response storage.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::ports::{
    IdempotencyMetricLabels, IdempotencyMetrics, IdempotencyStore, IdempotencyStoreError,
    NoOpIdempotencyMetrics, RouteSubmissionRequest, RouteSubmissionResponse,
    RouteSubmissionService,
};
use super::{
    Error, IdempotencyKey, IdempotencyLookupResult, IdempotencyRecord, PayloadHash, UserId,
    canonicalize_and_hash,
};

/// Compute age bucket string from record creation time.
///
/// Age buckets are aligned to typical retry patterns for a 24-hour TTL:
/// - `0-1m`: Immediate retries (network issues)
/// - `1-5m`: Client-side backoff retries
/// - `5-30m`: Session recovery
/// - `30m-2h`: Tab refresh / browser restart
/// - `2h-6h`: Same-day return
/// - `6h-24h`: Next-day retry before TTL expiry
///
/// # Example
///
/// ```ignore
/// use chrono::{Duration, Utc};
/// let created = Utc::now() - Duration::seconds(90);
/// assert_eq!(calculate_age_bucket(created), "1-5m");
/// ```
fn calculate_age_bucket(created_at: DateTime<Utc>) -> String {
    let age = Utc::now() - created_at;
    let minutes = age.num_minutes();

    match minutes {
        0 => "0-1m".to_string(),
        1..=4 => "1-5m".to_string(),
        5..=29 => "5-30m".to_string(),
        30..=119 => "30m-2h".to_string(),
        120..=359 => "2h-6h".to_string(),
        _ => "6h-24h".to_string(),
    }
}

/// Compute anonymised user scope from user ID.
///
/// Returns the first 8 hexadecimal characters of the SHA-256 hash of the
/// user ID string. This provides:
/// - Privacy: Cannot reverse to the original user ID
/// - Low cardinality: Suitable for Prometheus labels
/// - Traceability: Same user always maps to the same label
///
/// # Example
///
/// ```ignore
/// let user_id = UserId::new("550e8400-e29b-41d4-a716-446655440000").unwrap();
/// let scope = user_scope_hash(&user_id);
/// assert_eq!(scope.len(), 8);
/// assert!(scope.chars().all(|c| c.is_ascii_hexdigit()));
/// ```
fn user_scope_hash(user_id: &UserId) -> String {
    let mut hasher = Sha256::new();
    hasher.update(user_id.as_ref().as_bytes());
    let hash = hasher.finalize();
    hex::encode(&hash[..4])
}

/// Concrete implementation of [`RouteSubmissionService`].
///
/// Orchestrates route submission with idempotency support by:
/// 1. Looking up idempotency keys in the store.
/// 2. Comparing payload hashes to detect conflicts.
/// 3. Generating request IDs for new submissions.
/// 4. Storing responses for future replay.
/// 5. Recording metrics for observability.
///
/// The `M` type parameter allows injection of a metrics recorder. When metrics
/// are not needed (e.g., in tests), use [`NoOpIdempotencyMetrics`] as the default.
pub struct RouteSubmissionServiceImpl<S, M = NoOpIdempotencyMetrics> {
    idempotency_store: Arc<S>,
    idempotency_metrics: Arc<M>,
}

impl<S> RouteSubmissionServiceImpl<S, NoOpIdempotencyMetrics>
where
    S: IdempotencyStore,
{
    /// Create a new service with the given idempotency store and no-op metrics.
    ///
    /// Use this constructor when metrics recording is not required.
    pub fn with_noop_metrics(idempotency_store: Arc<S>) -> Self {
        Self {
            idempotency_store,
            idempotency_metrics: Arc::new(NoOpIdempotencyMetrics),
        }
    }
}

impl<S, M> RouteSubmissionServiceImpl<S, M>
where
    S: IdempotencyStore,
    M: IdempotencyMetrics,
{
    /// Create a new service with the given idempotency store and metrics recorder.
    pub fn new(idempotency_store: Arc<S>, idempotency_metrics: Arc<M>) -> Self {
        Self {
            idempotency_store,
            idempotency_metrics,
        }
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
                // TODO(#276): Dispatch to route queue here when integrated.
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
impl<S, M> RouteSubmissionService for RouteSubmissionServiceImpl<S, M>
where
    S: IdempotencyStore,
    M: IdempotencyMetrics,
{
    async fn submit(
        &self,
        request: RouteSubmissionRequest,
    ) -> Result<RouteSubmissionResponse, Error> {
        // If no idempotency key, proceed without tracking (skip hash computation).
        // Record a miss metric since no idempotency key means a new request.
        let Some(idempotency_key) = request.idempotency_key else {
            let labels = IdempotencyMetricLabels {
                user_scope: user_scope_hash(&request.user_id),
                age_bucket: None,
            };
            // Fire-and-forget: metrics errors should not fail the request.
            let _ = self.idempotency_metrics.record_miss(&labels).await;

            let request_id = Uuid::new_v4();
            // TODO(#276): Dispatch to route queue here when integrated.
            return Ok(RouteSubmissionResponse::accepted(request_id));
        };

        // Compute payload hash only when idempotency key is present.
        let payload_hash = canonicalize_and_hash(&request.payload);

        // Look up existing record for this key (scoped to user).
        let lookup_result = self
            .idempotency_store
            .lookup(&idempotency_key, &request.user_id, &payload_hash)
            .await
            .map_err(map_store_error)?;

        match lookup_result {
            IdempotencyLookupResult::NotFound => {
                // Record miss metric for new idempotent request.
                let labels = IdempotencyMetricLabels {
                    user_scope: user_scope_hash(&request.user_id),
                    age_bucket: None,
                };
                let _ = self.idempotency_metrics.record_miss(&labels).await;

                self.handle_new_request(idempotency_key, payload_hash, request.user_id)
                    .await
            }
            IdempotencyLookupResult::MatchingPayload(record) => {
                // Record hit metric for replayed response.
                let labels = IdempotencyMetricLabels {
                    user_scope: user_scope_hash(&request.user_id),
                    age_bucket: Some(calculate_age_bucket(record.created_at)),
                };
                let _ = self.idempotency_metrics.record_hit(&labels).await;

                let stored_response = Self::deserialize_stored_response(record.response_snapshot)?;
                Ok(RouteSubmissionResponse::replayed(
                    stored_response.request_id,
                ))
            }
            IdempotencyLookupResult::ConflictingPayload(record) => {
                // Record conflict metric for payload mismatch.
                let labels = IdempotencyMetricLabels {
                    user_scope: user_scope_hash(&request.user_id),
                    age_bucket: Some(calculate_age_bucket(record.created_at)),
                };
                let _ = self.idempotency_metrics.record_conflict(&labels).await;

                Err(Error::conflict(
                    "idempotency key already used with different payload",
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests;
