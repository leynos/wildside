//! HTTP request DTOs and conversion into the route-submission port.

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
    pub preferences: Option<RoutePreferencesDto>,
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
    pub lat: f64,
    /// Longitude in decimal degrees.
    pub lng: f64,
}

/// HTTP representation of optional route-generation preferences.
#[derive(Debug, Clone, Default, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoutePreferencesDto {
    /// Routing mode, such as `walking`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// Theme names used to bias route generation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub themes: Option<Vec<String>>,
    /// Theme identifiers used to bias route generation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_ids: Option<Vec<String>>,
    /// Interest-theme identifiers used to bias route generation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interest_theme_ids: Option<Vec<String>>,
    /// Route features that should be avoided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avoid: Option<Vec<String>>,
    /// Whether routes should avoid stairs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avoid_stairs: Option<bool>,
}

fn deserialize_preferences<'de, D>(deserializer: D) -> Result<Option<RoutePreferencesDto>, D::Error>
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
            preferences: request.preferences.map(Into::into),
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

impl From<RoutePreferencesDto> for RoutePreferences {
    fn from(preferences: RoutePreferencesDto) -> Self {
        Self {
            mode: preferences.mode,
            themes: preferences.themes,
            theme_ids: preferences.theme_ids,
            interest_theme_ids: preferences.interest_theme_ids,
            avoid: preferences.avoid,
            avoid_stairs: preferences.avoid_stairs,
        }
    }
}
