//! Theme read-model entity.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::CatalogueValidationError;
use super::image_asset::ImageAsset;
use super::validation::{
    ensure_non_negative, ensure_non_negative_range, ensure_valid_rating, validate_slug,
};
use crate::domain::localization::LocalizationMap;
use crate::domain::semantic_icon_identifier::SemanticIconIdentifier;

/// Input payload for [`Theme::new`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct ThemeDraft {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: SemanticIconIdentifier,
    pub localizations: LocalizationMap,
    pub image: ImageAsset,
    pub walk_count: i32,
    pub distance_range_metres: [i32; 2],
    pub rating: f32,
}

/// Theme card for Explore catalogue snapshots.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct Theme {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: SemanticIconIdentifier,
    pub localizations: LocalizationMap,
    pub image: ImageAsset,
    pub walk_count: i32,
    pub distance_range_metres: [i32; 2],
    pub rating: f32,
}

impl Theme {
    /// Validate and construct a theme card.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::collections::BTreeMap;
    ///
    /// use backend::domain::{
    ///     ImageAsset, LocalizationMap, LocalizedStringSet, SemanticIconIdentifier, Theme,
    ///     ThemeDraft,
    /// };
    /// use uuid::Uuid;
    ///
    /// let mut values = BTreeMap::new();
    /// values.insert(
    ///     "en-GB".to_owned(),
    ///     LocalizedStringSet::new("Coastal", Some("Coast".to_owned()), None),
    /// );
    /// let draft = ThemeDraft {
    ///     id: Uuid::new_v4(),
    ///     slug: "coastal".to_owned(),
    ///     icon_key: SemanticIconIdentifier::new("theme:coastal").expect("valid icon"),
    ///     localizations: LocalizationMap::new(values).expect("valid localization"),
    ///     image: ImageAsset::new("https://example.test/theme.jpg", "Coastal theme")
    ///         .expect("valid image"),
    ///     walk_count: 10,
    ///     distance_range_metres: [1_000, 5_000],
    ///     rating: 4.2,
    /// };
    ///
    /// let theme = Theme::new(draft).expect("valid theme");
    /// assert_eq!(theme.slug, "coastal");
    /// ```
    pub fn new(draft: ThemeDraft) -> Result<Self, CatalogueValidationError> {
        let slug = validate_slug(draft.slug, "theme.slug")?;
        ensure_non_negative(draft.walk_count, "theme.walk_count")?;
        ensure_non_negative_range(draft.distance_range_metres, "theme.distance_range_metres")?;
        ensure_valid_rating(draft.rating, "theme.rating")?;

        Ok(Self {
            id: draft.id,
            slug,
            icon_key: draft.icon_key,
            localizations: draft.localizations,
            image: draft.image,
            walk_count: draft.walk_count,
            distance_range_metres: draft.distance_range_metres,
            rating: draft.rating,
        })
    }
}
