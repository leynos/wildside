//! Versioned route-generation job payloads.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::ports::RouteSubmissionRequest;
use crate::domain::ports::define_port_error;
use crate::domain::{IdempotencyKey, UserId};

/// Versioned envelope for route-generation jobs.
///
/// Adding a field to an existing variant requires cutting a new `V2` variant.
/// Do not relax `deny_unknown_fields` on an existing variant.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "v")]
pub enum GenerateRouteJob {
    /// Version 1 route-generation payload.
    #[serde(rename = "v1")]
    V1(GenerateRouteJobV1),
}

/// Version 1 payload for `GenerateRouteJob`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GenerateRouteJobV1 {
    /// Stable identifier for this submission, used for trace correlation.
    pub request_id: Uuid,
    /// Optional idempotency key supplied by the client.
    pub idempotency_key: Option<IdempotencyKey>,
    /// Authenticated user owning the request.
    pub user_id: UserId,
    /// Origin location identifier or coordinates, as supplied by the API.
    pub origin: serde_json::Value,
    /// Destination location identifier or coordinates.
    pub destination: serde_json::Value,
    /// Optional preference payload.
    #[serde(default)]
    pub preferences: Option<serde_json::Value>,
    /// Wall-clock time at which the job was built and enqueued.
    pub enqueued_at: DateTime<Utc>,
}

impl GenerateRouteJob {
    /// Build a V1 route-generation job from validated pieces.
    #[expect(
        clippy::too_many_arguments,
        reason = "the approved V1 payload constructor mirrors the persisted schema fields"
    )]
    pub fn v1(
        request_id: Uuid,
        idempotency_key: Option<IdempotencyKey>,
        user_id: UserId,
        origin: serde_json::Value,
        destination: serde_json::Value,
        preferences: Option<serde_json::Value>,
        enqueued_at: DateTime<Utc>,
    ) -> Self {
        Self::V1(GenerateRouteJobV1 {
            request_id,
            idempotency_key,
            user_id,
            origin,
            destination,
            preferences,
            enqueued_at,
        })
    }

    /// Build a route-generation job from the route-submission port request.
    ///
    /// # Errors
    ///
    /// Returns [`GenerateRouteJobBuildError::PayloadNotObject`] when the
    /// submission payload is not a JSON object. Returns
    /// [`GenerateRouteJobBuildError::PayloadMissingField`] when the payload
    /// omits `origin` or `destination`.
    pub fn try_from_submission(
        submission: &RouteSubmissionRequest,
        request_id: Uuid,
        enqueued_at: DateTime<Utc>,
    ) -> Result<Self, GenerateRouteJobBuildError> {
        let payload = submission
            .payload
            .as_object()
            .ok_or_else(GenerateRouteJobBuildError::payload_not_object)?;
        let origin = payload
            .get("origin")
            .cloned()
            .ok_or_else(|| GenerateRouteJobBuildError::payload_missing_field("origin"))?;
        let destination = payload
            .get("destination")
            .cloned()
            .ok_or_else(|| GenerateRouteJobBuildError::payload_missing_field("destination"))?;
        let preferences = payload.get("preferences").cloned();

        Ok(Self::v1(
            request_id,
            submission.idempotency_key.clone(),
            submission.user_id.clone(),
            origin,
            destination,
            preferences,
            enqueued_at,
        ))
    }
}

define_port_error! {
    /// Errors raised while building route-generation jobs from submissions.
    pub enum GenerateRouteJobBuildError {
        /// Submission payload was not a JSON object.
        PayloadNotObject => "route job payload must be a JSON object",
        /// Submission payload missed a required field.
        PayloadMissingField { field: &'static str } =>
            "route job payload is missing required field: {field}",
    }
}

#[cfg(test)]
mod tests;
