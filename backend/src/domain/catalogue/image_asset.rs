//! Image asset representation for catalogue cards.

use serde::{Deserialize, Serialize};

use super::CatalogueValidationError;

/// Image asset projection used by catalogue cards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct ImageAsset {
    pub url: String,
    pub alt: String,
}

impl ImageAsset {
    /// Create an image asset.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use backend::domain::ImageAsset;
    ///
    /// let image = ImageAsset::new("https://example.test/hero.jpg", "Route hero")
    ///     .expect("valid image asset");
    /// assert_eq!(image.url, "https://example.test/hero.jpg");
    /// assert_eq!(image.alt, "Route hero");
    /// ```
    pub fn new(
        url: impl Into<String>,
        alt: impl Into<String>,
    ) -> Result<Self, CatalogueValidationError> {
        let url = url.into();
        let alt = alt.into();
        if url.trim().is_empty() {
            return Err(CatalogueValidationError::EmptyField { field: "image.url" });
        }
        if alt.trim().is_empty() {
            return Err(CatalogueValidationError::EmptyField { field: "image.alt" });
        }
        Ok(Self { url, alt })
    }
}
