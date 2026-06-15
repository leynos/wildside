//! Validated bounding-box payloads shared by background jobs.

use serde::{Deserialize, Serialize};

/// WGS84 bounding-box validation errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BoundingBoxError {
    /// At least one coordinate was NaN or infinite.
    #[error("bounding box coordinates must be finite")]
    NonFinite,
    /// At least one longitude was outside `[-180.0, 180.0]`.
    #[error("bounding box longitude must be within [-180.0, 180.0]")]
    LongitudeOutOfRange,
    /// At least one latitude was outside `[-90.0, 90.0]`.
    #[error("bounding box latitude must be within [-90.0, 90.0]")]
    LatitudeOutOfRange,
    /// Latitude ordering was inverted or empty.
    #[error("bounding box latitude ordering must satisfy min_lat < max_lat")]
    InvertedOrdering,
    /// Longitude ordering indicates an antimeridian-wrapped box.
    #[error("antimeridian-wrapped bounding boxes are not supported in V1")]
    AntimeridianWrap,
}

/// WGS84 bounding box in `[min_lng, min_lat, max_lng, max_lat]` order.
///
/// Antimeridian-wrapped boxes are not supported in V1 (`min_lng` must be
/// strictly less than `max_lng`). Callers spanning the dateline must split the
/// box into two pieces client-side.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "[f64; 4]", into = "[f64; 4]")]
pub struct BoundingBox {
    coords: [f64; 4],
}

impl BoundingBox {
    /// Validate and construct a WGS84 bounding box.
    ///
    /// # Errors
    ///
    /// Returns [`BoundingBoxError`] when any coordinate is non-finite, out of
    /// range, or ordered in a way V1 does not support.
    pub fn new(
        min_lng: f64,
        min_lat: f64,
        max_lng: f64,
        max_lat: f64,
    ) -> Result<Self, BoundingBoxError> {
        let coords = [min_lng, min_lat, max_lng, max_lat];
        if coords.iter().any(|coord| !coord.is_finite()) {
            return Err(BoundingBoxError::NonFinite);
        }
        if !(-180.0..=180.0).contains(&min_lng) || !(-180.0..=180.0).contains(&max_lng) {
            return Err(BoundingBoxError::LongitudeOutOfRange);
        }
        if !(-90.0..=90.0).contains(&min_lat) || !(-90.0..=90.0).contains(&max_lat) {
            return Err(BoundingBoxError::LatitudeOutOfRange);
        }
        if min_lng >= max_lng {
            return Err(BoundingBoxError::AntimeridianWrap);
        }
        if min_lat >= max_lat {
            return Err(BoundingBoxError::InvertedOrdering);
        }

        Ok(Self { coords })
    }

    /// Return the stored coordinates in wire-format order.
    pub fn coords(&self) -> [f64; 4] {
        self.coords
    }
}

impl TryFrom<[f64; 4]> for BoundingBox {
    type Error = BoundingBoxError;

    fn try_from(value: [f64; 4]) -> Result<Self, Self::Error> {
        let [min_lng, min_lat, max_lng, max_lat] = value;
        Self::new(min_lng, min_lat, max_lng, max_lat)
    }
}

impl From<BoundingBox> for [f64; 4] {
    fn from(value: BoundingBox) -> Self {
        value.coords
    }
}
