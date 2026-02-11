//! Trending route highlight read-model entity.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::CatalogueValidationError;
use super::validation::validate_non_empty_field;
use crate::domain::localization::LocalizationMap;

/// Trending overlay metadata keyed to route summary identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct TrendingRouteHighlight {
    pub id: Uuid,
    pub route_summary_id: Uuid,
    pub trend_delta: String,
    pub subtitle_localizations: LocalizationMap,
}

impl TrendingRouteHighlight {
    /// Validate and construct a trending route highlight.
    pub fn new(
        id: Uuid,
        route_summary_id: Uuid,
        trend_delta: impl Into<String>,
        subtitle_localizations: LocalizationMap,
    ) -> Result<Self, CatalogueValidationError> {
        let trend_delta =
            validate_non_empty_field(trend_delta.into(), "trending_route_highlight.trend_delta")?;

        Ok(Self {
            id,
            route_summary_id,
            trend_delta,
            subtitle_localizations,
        })
    }
}
