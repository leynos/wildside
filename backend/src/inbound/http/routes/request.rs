//! HTTP request DTOs and conversion into the route-submission port.

use actix_web::web;
use serde::{Deserialize, Serialize};

use crate::domain::Error;
use crate::domain::ports::{
    RouteCoordinates, RouteLocation, RoutePreferences, RouteSubmissionPayload, deserialize_non_null,
};

/// Route generation request body.
#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteRequest {
    /// Origin location identifier or coordinates.
    pub origin: RouteLocationDto,
    /// Destination location identifier or coordinates.
    pub destination: RouteLocationDto,
    /// Optional route preferences.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_preferences"
    )]
    #[schema(nullable = false, value_type = RoutePreferencesSchema)]
    pub preferences: Option<RoutePreferences>,
}

/// OpenAPI schema for [`RoutePreferences`].
///
/// This mirror stays in the HTTP adapter so the domain request contract remains
/// independent of the OpenAPI framework.
#[derive(utoipa::ToSchema)]
#[schema(
    as = RoutePreferences,
    description = "Optional route-generation preferences supported by the HTTP contract.\n\nString values and list entries are bounded during deserialization so the\nsame limits apply to HTTP requests and persisted job payloads."
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[expect(
    dead_code,
    reason = "Used only for OpenAPI schema generation via utoipa at the HTTP adapter boundary"
)]
struct RoutePreferencesSchema {
    /// Routing mode, such as `walking`.
    #[schema(max_length = 64)]
    mode: Option<String>,
    /// Theme names used to bias route generation.
    #[schema(max_items = 64)]
    themes: Option<Vec<String>>,
    /// Theme identifiers used to bias route generation.
    #[schema(rename = "themeIds", max_items = 64)]
    theme_ids: Option<Vec<String>>,
    /// Interest-theme identifiers used to bias route generation.
    #[schema(rename = "interestThemeIds", max_items = 64)]
    interest_theme_ids: Option<Vec<String>>,
    /// Route features that should be avoided.
    #[schema(max_items = 64)]
    avoid: Option<Vec<String>>,
    /// Whether routes should avoid stairs.
    #[schema(rename = "avoidStairs")]
    avoid_stairs: Option<bool>,
}

/// HTTP representation of a route location.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
#[serde(untagged)]
pub enum RouteLocationDto {
    /// A saved-location or point-of-interest identifier.
    Identifier(String),
    /// Explicit coordinates.
    Coordinates(RouteCoordinatesDto),
}

/// HTTP representation of route coordinates.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RouteCoordinatesDto {
    /// Latitude in decimal degrees.
    #[schema(minimum = -90.0, maximum = 90.0)]
    pub lat: f64,
    /// Longitude in decimal degrees.
    #[schema(minimum = -180.0, maximum = 180.0)]
    pub lng: f64,
}

/// Configure JSON extraction failures as client request errors.
pub fn route_request_json_config() -> web::JsonConfig {
    web::JsonConfig::default()
        .error_handler(|_error, _request| Error::invalid_request("invalid JSON request").into())
}

fn deserialize_preferences<'de, D>(deserializer: D) -> Result<Option<RoutePreferences>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_non_null(deserializer, "preferences").map(Some)
}

impl TryFrom<RouteRequest> for RouteSubmissionPayload {
    type Error = Error;

    fn try_from(request: RouteRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            origin: request.origin.try_into()?,
            destination: request.destination.try_into()?,
            preferences: request.preferences,
        })
    }
}

impl TryFrom<RouteLocationDto> for RouteLocation {
    type Error = Error;

    fn try_from(location: RouteLocationDto) -> Result<Self, Self::Error> {
        Ok(match location {
            RouteLocationDto::Identifier(identifier) => Self::Identifier(identifier),
            RouteLocationDto::Coordinates(coordinates) => {
                Self::Coordinates(coordinates.try_into()?)
            }
        })
    }
}

impl From<RouteLocation> for RouteLocationDto {
    fn from(location: RouteLocation) -> Self {
        match location {
            RouteLocation::Identifier(identifier) => Self::Identifier(identifier),
            RouteLocation::Coordinates(coordinates) => Self::Coordinates(coordinates.into()),
        }
    }
}

impl<'de> Deserialize<'de> for RouteLocationDto {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        RouteLocation::deserialize(deserializer).map(Into::into)
    }
}

impl TryFrom<RouteCoordinatesDto> for RouteCoordinates {
    type Error = Error;

    fn try_from(coordinates: RouteCoordinatesDto) -> Result<Self, Self::Error> {
        Self::new(coordinates.lat, coordinates.lng)
    }
}

impl From<RouteCoordinates> for RouteCoordinatesDto {
    fn from(coordinates: RouteCoordinates) -> Self {
        Self {
            lat: coordinates.lat(),
            lng: coordinates.lng(),
        }
    }
}
