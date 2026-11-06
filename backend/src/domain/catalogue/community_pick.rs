//! Community pick read-model entity.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::CatalogueValidationError;
use super::image_asset::ImageAsset;
use super::validation::{ensure_non_negative, ensure_valid_rating, validate_non_empty_field};
use crate::domain::localization::LocalizationMap;

/// Input payload for [`CommunityPick::new`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct CommunityPickDraft {
    /// Unique identifier for the pick card.
    pub id: Uuid,
    /// Route summary the pick links to, when the card is backed by one.
    pub route_summary_id: Option<Uuid>,
    /// Identifier of the user who curated the pick, when known.
    pub user_id: Option<Uuid>,
    /// Localized copy for the card, keyed by locale code.
    pub localizations: LocalizationMap,
    /// Curator name shown on the card; must not be blank.
    pub curator_display_name: String,
    /// Avatar image shown beside the curator's name.
    pub curator_avatar: ImageAsset,
    /// Average rating, from 0.0 to 5.0 inclusive.
    pub rating: f32,
    /// Route length in metres; must not be negative.
    pub distance_metres: i32,
    /// Expected duration in seconds; must not be negative.
    pub duration_seconds: i32,
    /// Number of times the pick has been saved; must not be negative.
    pub saves: i32,
}

/// Curated community pick card in Explore snapshots.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct CommunityPick {
    id: Uuid,
    route_summary_id: Option<Uuid>,
    user_id: Option<Uuid>,
    localizations: LocalizationMap,
    curator_display_name: String,
    curator_avatar: ImageAsset,
    rating: f32,
    distance_metres: i32,
    duration_seconds: i32,
    saves: i32,
}

impl CommunityPick {
    /// Validate and construct a community pick card.
    pub fn new(draft: CommunityPickDraft) -> Result<Self, CatalogueValidationError> {
        Self::try_from(draft)
    }

    /// Return the pick's unique identifier.
    pub fn id(&self) -> Uuid {
        self.id
    }
    /// Return the linked route summary identifier, if the pick has one.
    pub fn route_summary_id(&self) -> Option<Uuid> {
        self.route_summary_id
    }
    /// Return the curating user's identifier, if recorded.
    pub fn user_id(&self) -> Option<Uuid> {
        self.user_id
    }
    /// Borrow the localized card copy keyed by locale code.
    pub fn localizations(&self) -> &LocalizationMap {
        &self.localizations
    }
    /// Borrow the validated, non-blank curator name.
    pub fn curator_display_name(&self) -> &str {
        self.curator_display_name.as_str()
    }
    /// Borrow the curator's avatar image asset.
    pub fn curator_avatar(&self) -> &ImageAsset {
        &self.curator_avatar
    }
    /// Return the average rating, between 0.0 and 5.0 inclusive.
    pub fn rating(&self) -> f32 {
        self.rating
    }
    /// Return the route length in metres.
    pub fn distance_metres(&self) -> i32 {
        self.distance_metres
    }
    /// Return the expected duration in seconds.
    pub fn duration_seconds(&self) -> i32 {
        self.duration_seconds
    }
    /// Return how many times the pick has been saved.
    pub fn saves(&self) -> i32 {
        self.saves
    }
}

impl TryFrom<CommunityPickDraft> for CommunityPick {
    type Error = CatalogueValidationError;

    fn try_from(draft: CommunityPickDraft) -> Result<Self, Self::Error> {
        let curator_display_name = validate_non_empty_field(
            draft.curator_display_name,
            "community_pick.curator_display_name",
        )?;
        ensure_valid_rating(draft.rating, "community_pick.rating")?;
        ensure_non_negative(draft.distance_metres, "community_pick.distance_metres")?;
        ensure_non_negative(draft.duration_seconds, "community_pick.duration_seconds")?;
        ensure_non_negative(draft.saves, "community_pick.saves")?;

        Ok(Self {
            id: draft.id,
            route_summary_id: draft.route_summary_id,
            user_id: draft.user_id,
            localizations: draft.localizations,
            curator_display_name,
            curator_avatar: draft.curator_avatar,
            rating: draft.rating,
            distance_metres: draft.distance_metres,
            duration_seconds: draft.duration_seconds,
            saves: draft.saves,
        })
    }
}

impl<'de> Deserialize<'de> for CommunityPick {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        CommunityPickDraft::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}
