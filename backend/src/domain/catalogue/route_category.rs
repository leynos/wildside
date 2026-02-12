//! Route category read-model entity.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::CatalogueValidationError;
use super::validation::{ensure_non_negative, validate_slug};
use crate::domain::localization::LocalizationMap;
use crate::domain::semantic_icon_identifier::SemanticIconIdentifier;

/// Input payload for [`RouteCategory::new`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct RouteCategoryDraft {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: SemanticIconIdentifier,
    pub localizations: LocalizationMap,
    pub route_count: i32,
}

/// Route category entry for catalogue browsing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct RouteCategory {
    id: Uuid,
    slug: String,
    icon_key: SemanticIconIdentifier,
    localizations: LocalizationMap,
    route_count: i32,
}

impl RouteCategory {
    /// Validate and construct a route category.
    pub fn new(draft: RouteCategoryDraft) -> Result<Self, CatalogueValidationError> {
        Self::try_from(draft)
    }

    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn slug(&self) -> &str {
        self.slug.as_str()
    }
    pub fn icon_key(&self) -> &SemanticIconIdentifier {
        &self.icon_key
    }
    pub fn localizations(&self) -> &LocalizationMap {
        &self.localizations
    }
    pub fn route_count(&self) -> i32 {
        self.route_count
    }
}

impl TryFrom<RouteCategoryDraft> for RouteCategory {
    type Error = CatalogueValidationError;

    fn try_from(draft: RouteCategoryDraft) -> Result<Self, Self::Error> {
        let slug = validate_slug(draft.slug, "route_category.slug")?;
        ensure_non_negative(draft.route_count, "route_category.route_count")?;

        Ok(Self {
            id: draft.id,
            slug,
            icon_key: draft.icon_key,
            localizations: draft.localizations,
            route_count: draft.route_count,
        })
    }
}

impl<'de> Deserialize<'de> for RouteCategory {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        RouteCategoryDraft::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}
