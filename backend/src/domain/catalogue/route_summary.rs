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
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct RouteSummary {
    id: Uuid,
    route_id: Uuid,
    category_id: Uuid,
    theme_id: Uuid,
    slug: Option<String>,
    localizations: LocalizationMap,
    hero_image: ImageAsset,
    distance_metres: i32,
    duration_seconds: i32,
    rating: f32,
    badge_ids: Vec<Uuid>,
    difficulty: String,
    interest_theme_ids: Vec<Uuid>,
}

impl RouteSummary {
    /// Validate and construct a route summary card.
    pub fn new(draft: RouteSummaryDraft) -> Result<Self, CatalogueValidationError> {
        Self::try_from(draft)
    }

    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn route_id(&self) -> Uuid {
        self.route_id
    }
    pub fn category_id(&self) -> Uuid {
        self.category_id
    }
    pub fn theme_id(&self) -> Uuid {
        self.theme_id
    }
    pub fn slug(&self) -> Option<&str> {
        self.slug.as_deref()
    }
    pub fn localizations(&self) -> &LocalizationMap {
        &self.localizations
    }
    pub fn hero_image(&self) -> &ImageAsset {
        &self.hero_image
    }
    pub fn distance_metres(&self) -> i32 {
        self.distance_metres
    }
    pub fn duration_seconds(&self) -> i32 {
        self.duration_seconds
    }
    pub fn rating(&self) -> f32 {
        self.rating
    }
    pub fn badge_ids(&self) -> &[Uuid] {
        self.badge_ids.as_slice()
    }
    pub fn difficulty(&self) -> &str {
        self.difficulty.as_str()
    }
    pub fn interest_theme_ids(&self) -> &[Uuid] {
        self.interest_theme_ids.as_slice()
    }
}

impl TryFrom<RouteSummaryDraft> for RouteSummary {
    type Error = CatalogueValidationError;

    fn try_from(draft: RouteSummaryDraft) -> Result<Self, Self::Error> {
        let slug = draft
            .slug
            .map(|value| validate_slug(value, "route_summary.slug"))
            .transpose()?;
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

impl<'de> Deserialize<'de> for RouteSummary {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        RouteSummaryDraft::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}
