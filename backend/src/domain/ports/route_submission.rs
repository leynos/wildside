//! Driving port for route submission with idempotency support.
//!
//! The [`RouteSubmissionService`] coordinates route submission requests,
//! handling idempotency checking, job dispatch, and response storage. Inbound
//! adapters call this port to submit routes without knowing the backing
//! infrastructure details.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{Error, IdempotencyKey, UserId};

/// Request payload for route submission.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteSubmissionRequest {
    /// Optional idempotency key for safe retries.
    pub idempotency_key: Option<IdempotencyKey>,
    /// User making the request.
    pub user_id: UserId,
    /// The route generation payload (origin, destination, preferences, etc.).
    pub payload: serde_json::Value,
}

/// Status of a route submission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteSubmissionStatus {
    /// Request was accepted and queued for processing.
    Accepted,
    /// Request was a duplicate; replaying previous response.
    Replayed,
}

/// Response from a successful route submission.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteSubmissionResponse {
    /// Unique identifier for this route request.
    pub request_id: Uuid,
    /// Status indicating whether this was a new request or a replay.
    pub status: RouteSubmissionStatus,
}

impl RouteSubmissionResponse {
    /// Create a new response for an accepted request.
    pub fn accepted(request_id: Uuid) -> Self {
        Self {
            request_id,
            status: RouteSubmissionStatus::Accepted,
        }
    }

    /// Create a new response for a replayed request.
    pub fn replayed(request_id: Uuid) -> Self {
        Self {
            request_id,
            status: RouteSubmissionStatus::Replayed,
        }
    }
}

/// Driving port for route submission with idempotency.
///
/// Implementations coordinate:
/// 1. Idempotency key lookup (if key provided).
/// 2. Payload hash comparison for conflict detection.
/// 3. Job dispatch for new requests.
/// 4. Response storage for future replays.
#[async_trait]
pub trait RouteSubmissionService: Send + Sync {
    /// Submit a route generation request.
    ///
    /// # Idempotency Behaviour
    ///
    /// - If `idempotency_key` is `None`, proceeds without idempotency tracking.
    /// - If the key exists with matching payload, replays the previous response.
    /// - If the key exists with different payload, returns a conflict error.
    /// - If the key is new, queues the job and stores the response.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] for:
    /// - `Conflict`: Idempotency key reused with different payload.
    /// - `ServiceUnavailable`: Queue or store infrastructure failure.
    /// - `Internal`: Unexpected errors during processing.
    async fn submit(
        &self,
        request: RouteSubmissionRequest,
    ) -> Result<RouteSubmissionResponse, Error>;
}

/// Fixture implementation for testing.
///
/// Always accepts requests and generates a random request ID.
#[derive(Debug, Default)]
pub struct FixtureRouteSubmissionService;

#[async_trait]
impl RouteSubmissionService for FixtureRouteSubmissionService {
    async fn submit(
        &self,
        _request: RouteSubmissionRequest,
    ) -> Result<RouteSubmissionResponse, Error> {
        // In fixture mode, all requests are accepted with a random request ID.
        Ok(RouteSubmissionResponse::accepted(Uuid::new_v4()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fixture_service_accepts_requests() {
        let service = FixtureRouteSubmissionService;
        let request = RouteSubmissionRequest {
            idempotency_key: None,
            user_id: UserId::random(),
            payload: serde_json::json!({"origin": "A", "destination": "B"}),
        };

        let response = service
            .submit(request)
            .await
            .expect("submit should succeed");
        assert_eq!(response.status, RouteSubmissionStatus::Accepted);
    }

    #[tokio::test]
    async fn fixture_service_accepts_requests_with_idempotency_key() {
        let service = FixtureRouteSubmissionService;
        let request = RouteSubmissionRequest {
            idempotency_key: Some(IdempotencyKey::random()),
            user_id: UserId::random(),
            payload: serde_json::json!({"origin": "A", "destination": "B"}),
        };

        let response = service
            .submit(request)
            .await
            .expect("submit should succeed");
        assert_eq!(response.status, RouteSubmissionStatus::Accepted);
    }
}
