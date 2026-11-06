//! Offline bundle manifest domain types.
//!
//! This module models offline bundle metadata that the backend persists for
//! synchronization with the Progressive Web App (PWA). Tile bytes are not part
//! of this model; only manifest metadata lives in the domain.

use std::fmt;

mod bundle;
mod enums;
mod geometry;
#[cfg(test)]
mod tests;
mod validation;

pub use crate::domain::BoundingBox;
pub use bundle::{OfflineBundle, OfflineBundleDraft};
pub use enums::{
    OfflineBundleKind, OfflineBundleStatus, ParseOfflineBundleKindError,
    ParseOfflineBundleStatusError,
};
pub use geometry::ZoomRange;
pub use validation::normalize_device_id;

/// Validation errors raised by offline bundle constructors.
#[derive(Debug, Clone, PartialEq)]
pub enum OfflineValidationError {
    /// The device identifier was empty.
    EmptyDeviceId,
    /// The bundle bounds failed `BoundingBox` validation.
    BoundingBox(crate::domain::BoundingBoxError),
    /// The minimum zoom level was greater than the maximum.
    InvalidZoomRange {
        /// Minimum zoom level that was supplied.
        min_zoom: u8,
        /// Maximum zoom level that was supplied.
        max_zoom: u8,
    },
    /// Progress fell outside the inclusive range `[0.0, 1.0]`.
    InvalidProgress {
        /// Rejected progress value.
        progress: f32,
    },
    /// A route bundle was built without a `route_id`.
    MissingRouteIdForRouteBundle,
    /// A region bundle was built without a `region_id`.
    MissingRegionIdForRegionBundle,
    /// A region bundle was built with a `route_id`.
    UnexpectedRouteIdForRegionBundle,
    /// A route bundle was built with a `region_id`.
    UnexpectedRegionIdForRouteBundle,
    /// The `updated_at` timestamp precedes `created_at`.
    UpdatedBeforeCreated,
    /// The status and progress combination is not permitted.
    InvalidStatusProgress {
        /// Status that is incompatible with `progress`.
        status: OfflineBundleStatus,
        /// Progress value that is incompatible with `status`.
        progress: f32,
    },
}

impl fmt::Display for OfflineValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDeviceId => write!(f, "offline bundle device_id must not be empty"),
            Self::BoundingBox(error) => write!(f, "offline bundle bounds are invalid: {error}"),
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

impl From<crate::domain::BoundingBoxError> for OfflineValidationError {
    fn from(error: crate::domain::BoundingBoxError) -> Self {
        Self::BoundingBox(error)
    }
}
