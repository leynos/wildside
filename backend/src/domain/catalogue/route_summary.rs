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
    /// Unique identifier for the summary card.
    pub id: Uuid,
    /// Identifier of the route the card summarizes.
    pub route_id: Uuid,
    /// Identifier of the route category the route belongs to.
    pub category_id: Uuid,
    /// Identifier of the theme the route belongs to.
    pub theme_id: Uuid,
    /// Optional slug for the card; when present it is validated as a slug.
    pub slug: Option<String>,
    /// Localized card copy keyed by locale code.
    pub localizations: LocalizationMap,
    /// Hero image shown on the card.
    pub hero_image: ImageAsset,
    /// Route length in metres; must not be negative.
    pub distance_metres: i32,
    /// Expected duration in seconds; must not be negative.
    pub duration_seconds: i32,
    /// Average rating from 0.0 to 5.0 inclusive.
    pub rating: f32,
    /// Identifiers of the badges attached to the route.
    pub badge_ids: Vec<Uuid>,
    /// Difficulty label for the route; must not be blank.
    pub difficulty: String,
    /// Identifiers of the interest themes the route is associated with.
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

    /// Return the card's unique identifier.
    pub fn id(&self) -> Uuid {
        self.id
    }
    /// Return the identifier of the summarized route.
    pub fn route_id(&self) -> Uuid {
        self.route_id
    }
    /// Return the identifier of the route's category.
    pub fn category_id(&self) -> Uuid {
        self.category_id
    }
    /// Return the identifier of the route's theme.
    pub fn theme_id(&self) -> Uuid {
        self.theme_id
    }
    /// Return the validated slug, or `None` when the card has none.
    pub fn slug(&self) -> Option<&str> {
        self.slug.as_deref()
    }
    /// Borrow the localized card copy keyed by locale code.
    pub fn localizations(&self) -> &LocalizationMap {
        &self.localizations
    }
    /// Borrow the hero image shown on the card.
    pub fn hero_image(&self) -> &ImageAsset {
        &self.hero_image
    }
    /// Return the route length in metres.
    pub fn distance_metres(&self) -> i32 {
        self.distance_metres
    }
    /// Return the expected duration in seconds.
    pub fn duration_seconds(&self) -> i32 {
        self.duration_seconds
    }
    /// Return the average rating, from 0.0 to 5.0 inclusive.
    pub fn rating(&self) -> f32 {
        self.rating
    }
    /// Borrow the identifiers of the badges attached to the route.
    pub fn badge_ids(&self) -> &[Uuid] {
        self.badge_ids.as_slice()
    }
    /// Borrow the validated, non-blank difficulty label.
    pub fn difficulty(&self) -> &str {
        self.difficulty.as_str()
    }
    /// Borrow the identifiers of the interest themes the route is associated with.
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
