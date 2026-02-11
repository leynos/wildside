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
