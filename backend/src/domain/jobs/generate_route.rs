//! Versioned route-generation job payloads dispatched through the
//! [`RouteQueue`](crate::domain::ports::RouteQueue) port.
//!
//! This module provides strict V1 envelope serialization and deserialization,
//! accepts the typed route locations and preferences carried by
//! [`RouteSubmissionRequest`], and preserves their public JSON shape in the
//! durable queue payload.

use std::fmt;

use chrono::{DateTime, Utc};
use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

use crate::domain::ports::{
    RouteLocation, RoutePreferences, RouteSubmissionRequest, deserialize_non_null,
};
use crate::domain::{IdempotencyKey, UserId};

/// Versioned envelope for route-generation jobs.
///
/// Adding a field to an existing variant requires cutting a new `V2` variant.
/// Do not relax `deny_unknown_fields` on an existing variant.
#[derive(Clone, Debug, PartialEq, Serialize)]
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
    pub origin: RouteLocation,
    /// Destination location identifier or coordinates.
    #[serde(deserialize_with = "deserialize_destination")]
    pub destination: RouteLocation,
    /// Optional preference payload.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_preferences"
    )]
    pub preferences: Option<RoutePreferences>,
    /// Wall-clock time at which the job was built and enqueued.
    pub enqueued_at: DateTime<Utc>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GenerateRouteJobV1EnvelopeRaw {
    #[serde(rename = "v")]
    version: GenerateRouteJobV1Version,
    request_id: Uuid,
    idempotency_key: Option<IdempotencyKey>,
    user_id: UserId,
    #[serde(deserialize_with = "deserialize_origin")]
    origin: RouteLocation,
    #[serde(deserialize_with = "deserialize_destination")]
    destination: RouteLocation,
    #[serde(default, deserialize_with = "deserialize_preferences")]
    preferences: Option<RoutePreferences>,
    enqueued_at: DateTime<Utc>,
}

struct GenerateRouteJobV1Version;

impl<'de> Deserialize<'de> for GenerateRouteJobV1Version {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(GenerateRouteJobV1VersionVisitor)
    }
}

struct GenerateRouteJobV1VersionVisitor;

impl Visitor<'_> for GenerateRouteJobV1VersionVisitor {
    type Value = GenerateRouteJobV1Version;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the supported route-generation job version v1")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if value == "v1" {
            Ok(GenerateRouteJobV1Version)
        } else {
            Err(E::custom(
                "unsupported route-generation job envelope version",
            ))
        }
    }
}

impl<'de> Deserialize<'de> for GenerateRouteJob {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = GenerateRouteJobV1EnvelopeRaw::deserialize(deserializer)?;
        let GenerateRouteJobV1EnvelopeRaw {
            version: GenerateRouteJobV1Version,
            request_id,
            idempotency_key,
            user_id,
            origin,
            destination,
            preferences,
            enqueued_at,
        } = raw;

        Ok(Self::V1(GenerateRouteJobV1 {
            request_id,
            idempotency_key,
            user_id,
            origin,
            destination,
            preferences,
            enqueued_at,
        }))
    }
}

impl GenerateRouteJob {
    /// Build a V1 route-generation job from validated pieces.
    ///
    pub fn v1(payload: GenerateRouteJobV1) -> Self {
        Self::V1(payload)
    }

    /// Build a route-generation job from the route-submission port request.
    ///
    pub fn try_from_submission(
        submission: &RouteSubmissionRequest,
        request_id: Uuid,
        enqueued_at: DateTime<Utc>,
    ) -> Self {
        Self::v1(GenerateRouteJobV1 {
            request_id,
            idempotency_key: submission.idempotency_key.clone(),
            user_id: submission.user_id.clone(),
            origin: submission.payload.origin.clone(),
            destination: submission.payload.destination.clone(),
            preferences: submission.payload.preferences.clone(),
            enqueued_at,
        })
    }
}

fn deserialize_origin<'de, D>(deserializer: D) -> Result<RouteLocation, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_required_location(deserializer, "origin")
}

fn deserialize_destination<'de, D>(deserializer: D) -> Result<RouteLocation, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_required_location(deserializer, "destination")
}

fn deserialize_required_location<'de, D>(
    deserializer: D,
    field: &'static str,
) -> Result<RouteLocation, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_non_null(deserializer, field)
}

fn deserialize_preferences<'de, D>(deserializer: D) -> Result<Option<RoutePreferences>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_non_null(deserializer, "preferences").map(Some)
}

#[cfg(test)]
mod tests;
