//! Versioned route-generation job payloads dispatched through the
//! [`RouteQueue`](crate::domain::ports::RouteQueue) port.
//!
//! This module provides strict V1 envelope serialization and deserialization,
//! validates the required top-level `origin` and `destination` fields in a
//! [`RouteSubmissionRequest`]
//! payload, and deliberately keeps `origin`, `destination`, and `preferences`
//! as opaque JSON V1 fields.

use chrono::{DateTime, Utc};
use serde::de::Error as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
    #[serde(deserialize_with = "deserialize_origin")]
    pub origin: Value,
    /// Destination location identifier or coordinates.
    #[serde(deserialize_with = "deserialize_destination")]
    pub destination: Value,
    /// Optional preference payload.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_preferences"
    )]
    pub preferences: Option<Value>,
    /// Wall-clock time at which the job was built and enqueued.
    pub enqueued_at: DateTime<Utc>,
}

impl GenerateRouteJob {
    /// Build a V1 route-generation job from validated pieces.
    ///
    /// # Errors
    ///
    /// Returns [`GenerateRouteJobBuildError::PayloadMissingField`] when
    /// `origin` or `destination` is JSON `null`, and
    /// [`GenerateRouteJobBuildError::PayloadNullField`] when `preferences` is
    /// present as [`Value::Null`].
    pub fn v1(payload: GenerateRouteJobV1) -> Result<Self, GenerateRouteJobBuildError> {
        validate_required_payload(&payload.origin, "origin")?;
        validate_required_payload(&payload.destination, "destination")?;
        validate_preferences(&payload.preferences)?;
        Ok(Self::V1(payload))
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

        Self::v1(GenerateRouteJobV1 {
            request_id,
            idempotency_key: submission.idempotency_key.clone(),
            user_id: submission.user_id.clone(),
            origin,
            destination,
            preferences,
            enqueued_at,
        })
    }
}

fn validate_required_payload(
    value: &Value,
    field: &'static str,
) -> Result<(), GenerateRouteJobBuildError> {
    if value.is_null() {
        return Err(GenerateRouteJobBuildError::payload_missing_field(field));
    }
    Ok(())
}

fn validate_preferences(preferences: &Option<Value>) -> Result<(), GenerateRouteJobBuildError> {
    if preferences.as_ref().is_some_and(Value::is_null) {
        return Err(GenerateRouteJobBuildError::payload_null_field(
            "preferences",
        ));
    }
    Ok(())
}

fn deserialize_origin<'de, D>(deserializer: D) -> Result<Value, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_required_payload(deserializer, "origin")
}

fn deserialize_destination<'de, D>(deserializer: D) -> Result<Value, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_required_payload(deserializer, "destination")
}

fn deserialize_required_payload<'de, D>(
    deserializer: D,
    field: &'static str,
) -> Result<Value, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    validate_required_payload(&value, field).map_err(D::Error::custom)?;
    Ok(value)
}

fn deserialize_preferences<'de, D>(deserializer: D) -> Result<Option<Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    let preferences = Some(value);
    validate_preferences(&preferences).map_err(D::Error::custom)?;
    Ok(preferences)
}

define_port_error! {
    /// Errors raised while building route-generation jobs from submissions.
    pub enum GenerateRouteJobBuildError {
        /// Submission payload was not a JSON object.
        PayloadNotObject => "route job payload must be a JSON object",
        /// Submission payload missed a required field.
        PayloadMissingField { field: &'static str } =>
            "route job payload is missing required field: {field}",
        /// An optional payload field was explicitly JSON null.
        PayloadNullField { field: &'static str } =>
            "route job payload field must not be null: {field}",
    }
}

#[cfg(test)]
mod tests;
