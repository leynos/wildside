//! Route collection read-model entity.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::CatalogueValidationError;
use super::image_asset::ImageAsset;
use super::validation::{ensure_non_negative_range, validate_non_empty_field, validate_slug};
use crate::domain::localization::LocalizationMap;
use crate::domain::semantic_icon_identifier::SemanticIconIdentifier;

/// Input payload for [`RouteCollection::new`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct RouteCollectionDraft {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: SemanticIconIdentifier,
    pub localizations: LocalizationMap,
    pub lead_image: ImageAsset,
    pub map_preview: ImageAsset,
    pub distance_range_metres: [i32; 2],
    pub duration_range_seconds: [i32; 2],
    pub difficulty: String,
    pub route_ids: Vec<Uuid>,
}

/// Route collection card displayed in Explore snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct RouteCollection {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: SemanticIconIdentifier,
    pub localizations: LocalizationMap,
    pub lead_image: ImageAsset,
    pub map_preview: ImageAsset,
    pub distance_range_metres: [i32; 2],
    pub duration_range_seconds: [i32; 2],
    pub difficulty: String,
    pub route_ids: Vec<Uuid>,
}

impl RouteCollection {
    /// Validate and construct a route collection card.
    pub fn new(draft: RouteCollectionDraft) -> Result<Self, CatalogueValidationError> {
        let slug = validate_slug(draft.slug, "route_collection.slug")?;
        let difficulty = validate_non_empty_field(draft.difficulty, "route_collection.difficulty")?;
        ensure_non_negative_range(
            draft.distance_range_metres,
            "route_collection.distance_range_metres",
        )?;
        ensure_non_negative_range(
            draft.duration_range_seconds,
            "route_collection.duration_range_seconds",
        )?;

        Ok(Self {
            id: draft.id,
            slug,
            icon_key: draft.icon_key,
            localizations: draft.localizations,
            lead_image: draft.lead_image,
            map_preview: draft.map_preview,
            distance_range_metres: draft.distance_range_metres,
            duration_range_seconds: draft.duration_range_seconds,
            difficulty,
            route_ids: draft.route_ids,
        })
    }
}
