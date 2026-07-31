//! Shared WGS84 bounding-box value object and wire adapters.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

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
    #[error("antimeridian-wrapped bounding boxes are not supported")]
    AntimeridianWrap,
}

/// WGS84 bounding box in `[min_lng, min_lat, max_lng, max_lat]` order.
///
/// Antimeridian-wrapped and zero-area boxes are not supported. Callers
/// spanning the dateline must split the box into two pieces before building
/// this value object.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoundingBox {
    min_lng: f64,
    min_lat: f64,
    max_lng: f64,
    max_lat: f64,
}

impl BoundingBox {
    /// Validate and construct a WGS84 bounding box.
    ///
    /// # Errors
    ///
    /// Returns [`BoundingBoxError`] when any coordinate is non-finite, out of
    /// range, or ordered in an unsupported way.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use backend::domain::BoundingBox;
    ///
    /// let bounds = BoundingBox::new(-3.25, 55.92, -3.10, 56.01)?;
    /// assert_eq!(bounds.as_array(), [-3.25, 55.92, -3.10, 56.01]);
    /// # Ok::<(), backend::domain::BoundingBoxError>(())
    /// ```
    pub fn new(
        min_lng: f64,
        min_lat: f64,
        max_lng: f64,
        max_lat: f64,
    ) -> Result<Self, BoundingBoxError> {
        let coords = [min_lng, min_lat, max_lng, max_lat];
        if coords.iter().any(|coordinate| !coordinate.is_finite()) {
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

        Ok(Self {
            min_lng,
            min_lat,
            max_lng,
            max_lat,
        })
    }

    /// Return the coordinates in canonical array order.
    pub fn as_array(self) -> [f64; 4] {
        [self.min_lng, self.min_lat, self.max_lng, self.max_lat]
    }

    /// Return the coordinates in canonical array order.
    pub fn coords(&self) -> [f64; 4] {
        self.as_array()
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
        value.as_array()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BoundingBoxObject {
    min_lng: f64,
    min_lat: f64,
    max_lng: f64,
    max_lat: f64,
}

impl<'de> Deserialize<'de> for BoundingBox {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = BoundingBoxObject::deserialize(deserializer)?;
        Self::new(raw.min_lng, raw.min_lat, raw.max_lng, raw.max_lat)
            .map_err(serde::de::Error::custom)
    }
}

pub(crate) mod array_wire {
    //! Preserve the published array representation used by persisted jobs.

    use super::*;

    pub(crate) fn serialize<S>(value: &BoundingBox, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.as_array().serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<BoundingBox, D::Error>
    where
        D: Deserializer<'de>,
    {
        let coords = <[f64; 4]>::deserialize(deserializer)?;
        BoundingBox::try_from(coords).map_err(serde::de::Error::custom)
    }
}
