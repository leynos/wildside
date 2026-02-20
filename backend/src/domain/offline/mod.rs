//! Offline bundle manifest domain types.
//!
//! This module models offline bundle metadata that the backend persists for
//! synchronization with the Progressive Web App (PWA). Tile bytes are not part
//! of this model; only manifest metadata lives in the domain.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

use crate::domain::UserId;

mod enums;
#[cfg(test)]
mod tests;

pub use enums::{
    OfflineBundleKind, OfflineBundleStatus, ParseOfflineBundleKindError,
    ParseOfflineBundleStatusError,
};

/// Validation errors raised by offline bundle constructors.
#[derive(Debug, Clone, PartialEq)]
pub enum OfflineValidationError {
    EmptyDeviceId,
    InvalidBounds {
        field: &'static str,
        value: f64,
    },
    InvalidBoundsOrder,
    InvalidZoomRange {
        min_zoom: u8,
        max_zoom: u8,
    },
    InvalidProgress {
        progress: f32,
    },
    MissingRouteIdForRouteBundle,
    MissingRegionIdForRegionBundle,
    UnexpectedRouteIdForRegionBundle,
    UnexpectedRegionIdForRouteBundle,
    UpdatedBeforeCreated,
    InvalidStatusProgress {
        status: OfflineBundleStatus,
        progress: f32,
    },
}

impl fmt::Display for OfflineValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDeviceId => write!(f, "offline bundle device_id must not be empty"),
            Self::InvalidBounds { field, value } => {
                write!(
                    f,
                    "offline bundle bounds field {field} is out of range: {value}"
                )
            }
            Self::InvalidBoundsOrder => {
                write!(f, "offline bundle bounds must satisfy min <= max")
            }
            Self::InvalidZoomRange { min_zoom, max_zoom } => {
                write!(
                    f,
                    "offline bundle zoom range is invalid: [{min_zoom}, {max_zoom}]"
                )
            }
            Self::InvalidProgress { progress } => {
                write!(
                    f,
                    "offline bundle progress must be between 0.0 and 1.0: {progress}"
                )
            }
            Self::MissingRouteIdForRouteBundle => {
                write!(f, "route bundle must include route_id")
            }
            Self::MissingRegionIdForRegionBundle => {
                write!(f, "region bundle must include region_id")
            }
            Self::UnexpectedRouteIdForRegionBundle => {
                write!(f, "region bundle must not include route_id")
            }
            Self::UnexpectedRegionIdForRouteBundle => {
                write!(f, "route bundle must not include region_id")
            }
            Self::UpdatedBeforeCreated => {
                write!(f, "offline bundle updated_at must be >= created_at")
            }
            Self::InvalidStatusProgress { status, progress } => write!(
                f,
                "offline bundle status {status} is not compatible with progress {progress}"
            ),
        }
    }
}

impl std::error::Error for OfflineValidationError {}

/// Geographic bounds for an offline bundle.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoundingBox {
    min_lng: f64,
    min_lat: f64,
    max_lng: f64,
    max_lat: f64,
}

impl BoundingBox {
    pub fn new(
        min_lng: f64,
        min_lat: f64,
        max_lng: f64,
        max_lat: f64,
    ) -> Result<Self, OfflineValidationError> {
        validate_longitude(min_lng, "min_lng")?;
        validate_latitude(min_lat, "min_lat")?;
        validate_longitude(max_lng, "max_lng")?;
        validate_latitude(max_lat, "max_lat")?;

        if min_lng > max_lng || min_lat > max_lat {
            return Err(OfflineValidationError::InvalidBoundsOrder);
        }

        Ok(Self {
            min_lng,
            min_lat,
            max_lng,
            max_lat,
        })
    }

    pub fn as_array(self) -> [f64; 4] {
        [self.min_lng, self.min_lat, self.max_lng, self.max_lat]
    }
}

/// Inclusive zoom range for bundle tiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomRange {
    min_zoom: u8,
    max_zoom: u8,
}

impl ZoomRange {
    pub fn new(min_zoom: u8, max_zoom: u8) -> Result<Self, OfflineValidationError> {
        if min_zoom > max_zoom {
            return Err(OfflineValidationError::InvalidZoomRange { min_zoom, max_zoom });
        }

        Ok(Self { min_zoom, max_zoom })
    }

    pub fn min_zoom(&self) -> u8 {
        self.min_zoom
    }

    pub fn max_zoom(&self) -> u8 {
        self.max_zoom
    }
}

/// Input payload for [`OfflineBundle::new`].
#[derive(Debug, Clone)]
pub struct OfflineBundleDraft {
    pub id: Uuid,
    pub owner_user_id: Option<UserId>,
    pub device_id: String,
    pub kind: OfflineBundleKind,
    pub route_id: Option<Uuid>,
    pub region_id: Option<String>,
    pub bounds: BoundingBox,
    pub zoom_range: ZoomRange,
    pub estimated_size_bytes: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub status: OfflineBundleStatus,
    pub progress: f32,
}

/// Offline bundle manifest metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct OfflineBundle {
    id: Uuid,
    owner_user_id: Option<UserId>,
    device_id: String,
    kind: OfflineBundleKind,
    route_id: Option<Uuid>,
    region_id: Option<String>,
    bounds: BoundingBox,
    zoom_range: ZoomRange,
    estimated_size_bytes: u64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    status: OfflineBundleStatus,
    progress: f32,
}

impl OfflineBundle {
    pub fn new(draft: OfflineBundleDraft) -> Result<Self, OfflineValidationError> {
        Self::try_from(draft)
    }

    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn owner_user_id(&self) -> Option<&UserId> {
        self.owner_user_id.as_ref()
    }
    pub fn device_id(&self) -> &str {
        self.device_id.as_str()
    }
    pub fn kind(&self) -> OfflineBundleKind {
        self.kind
    }
    pub fn route_id(&self) -> Option<Uuid> {
        self.route_id
    }
    pub fn region_id(&self) -> Option<&str> {
        self.region_id.as_deref()
    }
    pub fn bounds(&self) -> BoundingBox {
        self.bounds
    }
    pub fn zoom_range(&self) -> ZoomRange {
        self.zoom_range
    }
    pub fn estimated_size_bytes(&self) -> u64 {
        self.estimated_size_bytes
    }
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }
    pub fn status(&self) -> OfflineBundleStatus {
        self.status
    }
    pub fn progress(&self) -> f32 {
        self.progress
    }
}

impl TryFrom<OfflineBundleDraft> for OfflineBundle {
    type Error = OfflineValidationError;

    fn try_from(draft: OfflineBundleDraft) -> Result<Self, Self::Error> {
        let device_id = validate_device_id(&draft.device_id)?;
        validate_progress(draft.progress)?;
        validate_timestamps(draft.created_at, draft.updated_at)?;
        validate_kind_specific(draft.kind, draft.route_id, draft.region_id.clone())?;
        validate_status_progress(draft.status, draft.progress)?;

        Ok(Self {
            id: draft.id,
            owner_user_id: draft.owner_user_id,
            device_id,
            kind: draft.kind,
            route_id: draft.route_id,
            region_id: draft.region_id.map(|value| value.trim().to_owned()),
            bounds: draft.bounds,
            zoom_range: draft.zoom_range,
            estimated_size_bytes: draft.estimated_size_bytes,
            created_at: draft.created_at,
            updated_at: draft.updated_at,
            status: draft.status,
            progress: draft.progress,
        })
    }
}

/// Validates and normalizes the bundle device identifier.
/// # Examples
/// `validate_device_id("  ios-phone  ")` returns `"ios-phone"`.
fn validate_device_id(device_id: &str) -> Result<String, OfflineValidationError> {
    let normalized = device_id.trim().to_owned();
    if normalized.is_empty() {
        return Err(OfflineValidationError::EmptyDeviceId);
    }
    Ok(normalized)
}

/// Validates that bundle progress is within the inclusive range [0.0, 1.0].
/// # Examples
/// `validate_progress(0.5)` succeeds, while `validate_progress(1.2)` fails.
fn validate_progress(progress: f32) -> Result<(), OfflineValidationError> {
    if !(0.0..=1.0).contains(&progress) {
        return Err(OfflineValidationError::InvalidProgress { progress });
    }
    Ok(())
}

/// Validates that update timestamps do not precede creation timestamps.
/// # Examples
/// `validate_timestamps(now, now)` succeeds, while `validate_timestamps(now, now - 1s)` fails.
fn validate_timestamps(
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
) -> Result<(), OfflineValidationError> {
    if updated_at < created_at {
        return Err(OfflineValidationError::UpdatedBeforeCreated);
    }
    Ok(())
}

/// Validates route-bundle requirements for route and region identifiers.
/// # Examples
/// A route bundle with `Some(route_id)` and `None` region id succeeds.
fn validate_route_bundle(
    route_id: Option<uuid::Uuid>,
    region_id: Option<String>,
) -> Result<(), OfflineValidationError> {
    if route_id.is_none() {
        return Err(OfflineValidationError::MissingRouteIdForRouteBundle);
    }
    if region_id.is_some() {
        return Err(OfflineValidationError::UnexpectedRegionIdForRouteBundle);
    }
    Ok(())
}

/// Validates region-bundle requirements for route and region identifiers.
/// # Examples
/// A region bundle with `None` route id and non-blank region id succeeds.
fn validate_region_bundle(
    route_id: Option<uuid::Uuid>,
    region_id: Option<String>,
) -> Result<(), OfflineValidationError> {
    let region_value = region_id.as_deref().map(str::trim).unwrap_or_default();
    if region_value.is_empty() {
        return Err(OfflineValidationError::MissingRegionIdForRegionBundle);
    }
    if route_id.is_some() {
        return Err(OfflineValidationError::UnexpectedRouteIdForRegionBundle);
    }
    Ok(())
}

/// Dispatches bundle-kind-specific validation for route and region identifiers.
/// # Examples
/// `validate_kind_specific` delegates to route or region validation by bundle kind.
fn validate_kind_specific(
    kind: OfflineBundleKind,
    route_id: Option<uuid::Uuid>,
    region_id: Option<String>,
) -> Result<(), OfflineValidationError> {
    match kind {
        OfflineBundleKind::Route => validate_route_bundle(route_id, region_id),
        OfflineBundleKind::Region => validate_region_bundle(route_id, region_id),
    }
}

fn validate_longitude(value: f64, field: &'static str) -> Result<(), OfflineValidationError> {
    if !value.is_finite() || !(-180.0..=180.0).contains(&value) {
        return Err(OfflineValidationError::InvalidBounds { field, value });
    }
    Ok(())
}

fn validate_latitude(value: f64, field: &'static str) -> Result<(), OfflineValidationError> {
    if !value.is_finite() || !(-90.0..=90.0).contains(&value) {
        return Err(OfflineValidationError::InvalidBounds { field, value });
    }
    Ok(())
}

fn validate_status_progress(
    status: OfflineBundleStatus,
    progress: f32,
) -> Result<(), OfflineValidationError> {
    let is_status_progress_valid = is_status_progress_valid(status, progress);

    if is_status_progress_valid {
        Ok(())
    } else {
        Err(OfflineValidationError::InvalidStatusProgress { status, progress })
    }
}

fn is_status_progress_valid(status: OfflineBundleStatus, progress: f32) -> bool {
    match status {
        OfflineBundleStatus::Queued => progress == 0.0,
        OfflineBundleStatus::Downloading => (0.0..1.0).contains(&progress),
        OfflineBundleStatus::Complete => progress == 1.0,
        OfflineBundleStatus::Failed => (0.0..=1.0).contains(&progress),
    }
}
