//! Offline bundle geometric value objects.

use serde::{Deserialize, Serialize};

use super::OfflineValidationError;

/// Geographic bounds for an offline bundle.
///
/// # Examples
///
/// ```rust,ignore
/// let bounds = backend::domain::BoundingBox::new(-3.25, 55.92, -3.10, 56.01)?;
/// assert_eq!(bounds.as_array(), [-3.25, 55.92, -3.10, 56.01]);
/// Ok::<(), backend::domain::OfflineValidationError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoundingBox {
    min_lng: f64,
    min_lat: f64,
    max_lng: f64,
    max_lat: f64,
}

impl BoundingBox {
    /// Creates a validated bounding box.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let ok = backend::domain::BoundingBox::new(-3.25, 55.92, -3.10, 56.01)?;
    /// assert_eq!(ok.as_array()[0], -3.25);
    /// let err = backend::domain::BoundingBox::new(-181.0, 55.92, -3.10, 56.01);
    /// assert!(err.is_err());
    /// Ok::<(), backend::domain::OfflineValidationError>(())
    /// ```
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

    /// Returns bounds as `[min_lng, min_lat, max_lng, max_lat]`.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let bounds = backend::domain::BoundingBox::new(-3.25, 55.92, -3.10, 56.01)?;
    /// assert_eq!(bounds.as_array(), [-3.25, 55.92, -3.10, 56.01]);
    /// Ok::<(), backend::domain::OfflineValidationError>(())
    /// ```
    pub fn as_array(self) -> [f64; 4] {
        [self.min_lng, self.min_lat, self.max_lng, self.max_lat]
    }
}

/// Inclusive zoom range for bundle tiles.
///
/// # Examples
///
/// ```rust,ignore
/// let zoom = backend::domain::ZoomRange::new(12, 16)?;
/// assert_eq!(zoom.min_zoom(), 12);
/// assert_eq!(zoom.max_zoom(), 16);
/// Ok::<(), backend::domain::OfflineValidationError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomRange {
    min_zoom: u8,
    max_zoom: u8,
}

impl ZoomRange {
    /// Creates a validated zoom range where `min_zoom <= max_zoom`.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let ok = backend::domain::ZoomRange::new(12, 16)?;
    /// assert_eq!(ok.min_zoom(), 12);
    /// let err = backend::domain::ZoomRange::new(16, 12);
    /// assert!(err.is_err());
    /// Ok::<(), backend::domain::OfflineValidationError>(())
    /// ```
    pub fn new(min_zoom: u8, max_zoom: u8) -> Result<Self, OfflineValidationError> {
        if min_zoom > max_zoom {
            return Err(OfflineValidationError::InvalidZoomRange { min_zoom, max_zoom });
        }

        Ok(Self { min_zoom, max_zoom })
    }

    /// Returns the inclusive minimum zoom level.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let zoom = backend::domain::ZoomRange::new(12, 16)?;
    /// assert_eq!(zoom.min_zoom(), 12);
    /// Ok::<(), backend::domain::OfflineValidationError>(())
    /// ```
    pub fn min_zoom(&self) -> u8 {
        self.min_zoom
    }

    /// Returns the inclusive maximum zoom level.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let zoom = backend::domain::ZoomRange::new(12, 16)?;
    /// assert_eq!(zoom.max_zoom(), 16);
    /// Ok::<(), backend::domain::OfflineValidationError>(())
    /// ```
    pub fn max_zoom(&self) -> u8 {
        self.max_zoom
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
