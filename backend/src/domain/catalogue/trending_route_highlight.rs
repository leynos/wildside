//! Trending route highlight read-model entity.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::CatalogueValidationError;
use super::validation::validate_non_empty_field;
use crate::domain::localization::LocalizationMap;

/// Input payload for [`TrendingRouteHighlight::new`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct TrendingRouteHighlightDraft {
    pub id: Uuid,
    pub route_summary_id: Uuid,
    pub trend_delta: String,
    pub subtitle_localizations: LocalizationMap,
}

/// Trending overlay metadata keyed to route summary identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct TrendingRouteHighlight {
    id: Uuid,
    route_summary_id: Uuid,
    trend_delta: String,
    subtitle_localizations: LocalizationMap,
}

impl TrendingRouteHighlight {
    /// Validate and construct a trending route highlight.
    pub fn new(
        id: Uuid,
        route_summary_id: Uuid,
        trend_delta: impl Into<String>,
        subtitle_localizations: LocalizationMap,
    ) -> Result<Self, CatalogueValidationError> {
        Self::try_from(TrendingRouteHighlightDraft {
            id,
            route_summary_id,
            trend_delta: trend_delta.into(),
            subtitle_localizations,
        })
    }

    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn route_summary_id(&self) -> Uuid {
        self.route_summary_id
    }
    pub fn trend_delta(&self) -> &str {
        self.trend_delta.as_str()
    }
    pub fn subtitle_localizations(&self) -> &LocalizationMap {
        &self.subtitle_localizations
    }
}

impl TryFrom<TrendingRouteHighlightDraft> for TrendingRouteHighlight {
    type Error = CatalogueValidationError;

    fn try_from(draft: TrendingRouteHighlightDraft) -> Result<Self, Self::Error> {
        let trend_delta =
            validate_non_empty_field(draft.trend_delta, "trending_route_highlight.trend_delta")?;

        Ok(Self {
            id: draft.id,
            route_summary_id: draft.route_summary_id,
            trend_delta,
            subtitle_localizations: draft.subtitle_localizations,
        })
    }
}

impl<'de> Deserialize<'de> for TrendingRouteHighlight {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        TrendingRouteHighlightDraft::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}
