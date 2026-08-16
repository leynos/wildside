//! Driving port for route submission with idempotency support.
//!
//! The [`RouteSubmissionService`] coordinates route submission requests,
//! handling idempotency checking, job dispatch, and response storage. Inbound
//! adapters call this port to submit routes without knowing the backing
//! infrastructure details.

use std::fmt;

use async_trait::async_trait;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

use super::route_preferences::RoutePreferences;
use crate::domain::{Error, IdempotencyKey, UserId};

/// A location accepted by route generation.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum RouteLocation {
    /// A stable saved-location or point-of-interest identifier.
    Identifier(String),
    /// Explicit decimal-degree coordinates.
    Coordinates(RouteCoordinates),
}

/// Decimal-degree coordinates used by a route endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RouteCoordinates {
    /// Latitude in decimal degrees.
    lat: f64,
    /// Longitude in decimal degrees.
    lng: f64,
}

impl RouteCoordinates {
    /// Construct validated WGS84 route coordinates.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use backend::domain::ports::RouteCoordinates;
    ///
    /// let coordinates = RouteCoordinates::new(51.5, -0.1)?;
    /// assert_eq!(coordinates.lat(), 51.5);
    /// # Ok::<(), backend::domain::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`Error`] when latitude or longitude is non-finite or outside
    /// the WGS84 ranges `[-90, 90]` and `[-180, 180]`, respectively.
    pub fn new(lat: f64, lng: f64) -> Result<Self, Error> {
        if !lat.is_finite() || !(-90.0..=90.0).contains(&lat) {
            return Err(Error::invalid_request(
                "route latitude must be finite and within [-90, 90]",
            ));
        }
        if !lng.is_finite() || !(-180.0..=180.0).contains(&lng) {
            return Err(Error::invalid_request(
                "route longitude must be finite and within [-180, 180]",
            ));
        }
        Ok(Self { lat, lng })
    }

    /// Return the latitude in decimal degrees.
    pub const fn lat(&self) -> f64 {
        self.lat
    }

    /// Return the longitude in decimal degrees.
    pub const fn lng(&self) -> f64 {
        self.lng
    }
}

struct RouteLocationVisitor;

impl<'de> Visitor<'de> for RouteLocationVisitor {
    type Value = RouteLocation;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a location identifier string or coordinate object")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(RouteLocation::Identifier(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(RouteLocation::Identifier(value))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut lat = None;
        let mut lng = None;
        while let Some(field) = map.next_key::<String>()? {
            match field.as_str() {
                "lat" if lat.is_none() => lat = Some(map.next_value()?),
                "lng" if lng.is_none() => lng = Some(map.next_value()?),
                "lat" => return Err(serde::de::Error::duplicate_field("lat")),
                "lng" => return Err(serde::de::Error::duplicate_field("lng")),
                _ => return Err(serde::de::Error::unknown_field(&field, &["lat", "lng"])),
            }
        }
        let lat = lat.ok_or_else(|| serde::de::Error::missing_field("lat"))?;
        let lng = lng.ok_or_else(|| serde::de::Error::missing_field("lng"))?;
        let coordinates = RouteCoordinates::new(lat, lng)
            .map_err(|error| serde::de::Error::custom(error.message()))?;
        Ok(RouteLocation::Coordinates(coordinates))
    }
}

impl<'de> Deserialize<'de> for RouteLocation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(RouteLocationVisitor)
    }
}

/// Typed route-generation payload shared by the inbound port and queued job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteSubmissionPayload {
    /// Origin location identifier or coordinates.
    pub origin: RouteLocation,
    /// Destination location identifier or coordinates.
    pub destination: RouteLocation,
    /// Optional route-generation preferences.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_route_preferences"
    )]
    pub preferences: Option<RoutePreferences>,
}

fn deserialize_route_preferences<'de, D>(
    deserializer: D,
) -> Result<Option<RoutePreferences>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_non_null(deserializer, "preferences").map(Some)
}

pub(crate) fn deserialize_non_null<'de, D, T>(
    deserializer: D,
    field: &'static str,
) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)?
        .ok_or_else(|| serde::de::Error::custom(format!("{field} must not be null")))
}

/// Request payload for route submission.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteSubmissionRequest {
    /// Optional idempotency key for safe retries.
    pub idempotency_key: Option<IdempotencyKey>,
    /// User making the request.
    pub user_id: UserId,
    /// Typed route-generation payload.
    pub payload: RouteSubmissionPayload,
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
    //! Regression coverage for this module.
    use super::*;

    fn route_payload() -> RouteSubmissionPayload {
        RouteSubmissionPayload {
            origin: RouteLocation::Identifier("A".to_owned()),
            destination: RouteLocation::Identifier("B".to_owned()),
            preferences: None,
        }
    }

    #[tokio::test]
    async fn fixture_service_accepts_requests() {
        let service = FixtureRouteSubmissionService;
        let request = RouteSubmissionRequest {
            idempotency_key: None,
            user_id: UserId::random(),
            payload: route_payload(),
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
            payload: route_payload(),
        };

        let response = service
            .submit(request)
            .await
            .expect("submit should succeed");
        assert_eq!(response.status, RouteSubmissionStatus::Accepted);
    }
}
