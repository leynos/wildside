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
    /// Unique identifier for the collection.
    pub id: Uuid,
    /// Slug identifying the collection; validated as a domain slug on construction.
    pub slug: String,
    /// Semantic icon identifier, such as `category:nature`, that the client resolves.
    pub icon_key: SemanticIconIdentifier,
    /// Localized card copy keyed by locale code.
    pub localizations: LocalizationMap,
    /// Hero image shown on the collection card.
    pub lead_image: ImageAsset,
    /// Map preview image shown alongside the collection.
    pub map_preview: ImageAsset,
    /// Distance range as `[min, max]` in metres; bounds must be non-negative and ordered.
    pub distance_range_metres: [i32; 2],
    /// Duration range as `[min, max]` in seconds; bounds must be non-negative and ordered.
    pub duration_range_seconds: [i32; 2],
    /// Difficulty label for the collection; must not be blank.
    pub difficulty: String,
    /// Identifiers of the routes grouped by this collection.
    pub route_ids: Vec<Uuid>,
}

/// Route collection card displayed in Explore snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct RouteCollection {
    id: Uuid,
    slug: String,
    icon_key: SemanticIconIdentifier,
    localizations: LocalizationMap,
    lead_image: ImageAsset,
    map_preview: ImageAsset,
    distance_range_metres: [i32; 2],
    duration_range_seconds: [i32; 2],
    difficulty: String,
    route_ids: Vec<Uuid>,
}

impl RouteCollection {
    /// Validate and construct a route collection card.
    pub fn new(draft: RouteCollectionDraft) -> Result<Self, CatalogueValidationError> {
        Self::try_from(draft)
    }

    /// Return the collection's unique identifier.
    pub fn id(&self) -> Uuid {
        self.id
    }
    /// Borrow the validated slug identifying the collection.
    pub fn slug(&self) -> &str {
        self.slug.as_str()
    }
    /// Borrow the semantic icon identifier for the collection card.
    pub fn icon_key(&self) -> &SemanticIconIdentifier {
        &self.icon_key
    }
    /// Borrow the localized card copy keyed by locale code.
    pub fn localizations(&self) -> &LocalizationMap {
        &self.localizations
    }
    /// Borrow the hero image shown on the collection card.
    pub fn lead_image(&self) -> &ImageAsset {
        &self.lead_image
    }
    /// Borrow the map preview image shown alongside the collection.
    pub fn map_preview(&self) -> &ImageAsset {
        &self.map_preview
    }
    /// Borrow the `[min, max]` route distance range in metres.
    pub fn distance_range_metres(&self) -> &[i32; 2] {
        &self.distance_range_metres
    }
    /// Borrow the `[min, max]` route duration range in seconds.
    pub fn duration_range_seconds(&self) -> &[i32; 2] {
        &self.duration_range_seconds
    }
    /// Borrow the validated, non-blank difficulty label.
    pub fn difficulty(&self) -> &str {
        self.difficulty.as_str()
    }
    /// Borrow the identifiers of the routes grouped by this collection.
    pub fn route_ids(&self) -> &[Uuid] {
        self.route_ids.as_slice()
    }
}

impl TryFrom<RouteCollectionDraft> for RouteCollection {
    type Error = CatalogueValidationError;

    fn try_from(draft: RouteCollectionDraft) -> Result<Self, Self::Error> {
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

impl<'de> Deserialize<'de> for RouteCollection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        RouteCollectionDraft::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}
