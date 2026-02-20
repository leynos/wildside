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

pub use bundle::{OfflineBundle, OfflineBundleDraft};
pub use enums::{
    OfflineBundleKind, OfflineBundleStatus, ParseOfflineBundleKindError,
    ParseOfflineBundleStatusError,
};
pub use geometry::{BoundingBox, ZoomRange};

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
