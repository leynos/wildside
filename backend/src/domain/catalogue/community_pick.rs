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
    pub id: Uuid,
    pub route_summary_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub localizations: LocalizationMap,
    pub curator_display_name: String,
    pub curator_avatar: ImageAsset,
    pub rating: f32,
    pub distance_metres: i32,
    pub duration_seconds: i32,
    pub saves: i32,
}

/// Curated community pick card in Explore snapshots.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct CommunityPick {
    pub id: Uuid,
    pub route_summary_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub localizations: LocalizationMap,
    pub curator_display_name: String,
    pub curator_avatar: ImageAsset,
    pub rating: f32,
    pub distance_metres: i32,
    pub duration_seconds: i32,
    pub saves: i32,
}

impl CommunityPick {
    /// Validate and construct a community pick card.
    pub fn new(draft: CommunityPickDraft) -> Result<Self, CatalogueValidationError> {
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
