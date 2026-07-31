//! Offline bundle zoom-range value object.

use serde::{Deserialize, Serialize};

use super::OfflineValidationError;

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
