//! Route summary read-model entity.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::CatalogueValidationError;
use super::image_asset::ImageAsset;
use super::validation::{
    ensure_non_negative, ensure_valid_rating, validate_non_empty_field, validate_slug,
};
use crate::domain::localization::LocalizationMap;

/// Input payload for [`RouteSummary::new`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct RouteSummaryDraft {
    pub id: Uuid,
    pub route_id: Uuid,
    pub category_id: Uuid,
    pub theme_id: Uuid,
    pub slug: Option<String>,
    pub localizations: LocalizationMap,
    pub hero_image: ImageAsset,
    pub distance_metres: i32,
    pub duration_seconds: i32,
    pub rating: f32,
    pub badge_ids: Vec<Uuid>,
    pub difficulty: String,
    pub interest_theme_ids: Vec<Uuid>,
}

/// Route summary projection rendered as an Explore card.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct RouteSummary {
    pub id: Uuid,
    pub route_id: Uuid,
    pub category_id: Uuid,
    pub theme_id: Uuid,
    pub slug: Option<String>,
    pub localizations: LocalizationMap,
    pub hero_image: ImageAsset,
    pub distance_metres: i32,
    pub duration_seconds: i32,
    pub rating: f32,
    pub badge_ids: Vec<Uuid>,
    pub difficulty: String,
    pub interest_theme_ids: Vec<Uuid>,
}

impl RouteSummary {
    /// Validate and construct a route summary card.
    pub fn new(draft: RouteSummaryDraft) -> Result<Self, CatalogueValidationError> {
        let slug = match draft.slug {
            Some(value) => Some(validate_slug(value, "route_summary.slug")?),
            None => None,
        };
        ensure_non_negative(draft.distance_metres, "route_summary.distance_metres")?;
        ensure_non_negative(draft.duration_seconds, "route_summary.duration_seconds")?;
        ensure_valid_rating(draft.rating, "route_summary.rating")?;
        let difficulty = validate_non_empty_field(draft.difficulty, "route_summary.difficulty")?;

        Ok(Self {
            id: draft.id,
            route_id: draft.route_id,
            category_id: draft.category_id,
            theme_id: draft.theme_id,
            slug,
            localizations: draft.localizations,
            hero_image: draft.hero_image,
            distance_metres: draft.distance_metres,
            duration_seconds: draft.duration_seconds,
            rating: draft.rating,
            badge_ids: draft.badge_ids,
            difficulty,
            interest_theme_ids: draft.interest_theme_ids,
        })
    }
}
