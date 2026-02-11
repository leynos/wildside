//! Diesel insertable rows used by catalogue and descriptor ingestion adapters.

use diesel::prelude::*;
use uuid::Uuid;

use crate::outbound::persistence::schema::{
    badges, community_picks, interest_themes, route_categories, route_collections, route_summaries,
    safety_presets, safety_toggles, tags, themes, trending_route_highlights,
};

/// Insertable row for route categories.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = route_categories)]
pub(crate) struct NewRouteCategoryRow {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: String,
    pub localizations: serde_json::Value,
    pub route_count: i32,
}

/// Insertable row for themes.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = themes)]
pub(crate) struct NewThemeRow {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: String,
    pub localizations: serde_json::Value,
    pub image: serde_json::Value,
    pub walk_count: i32,
    pub distance_range_metres: Vec<i32>,
    pub rating: f32,
}

/// Insertable row for route collections.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = route_collections)]
pub(crate) struct NewRouteCollectionRow {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: String,
    pub localizations: serde_json::Value,
    pub lead_image: serde_json::Value,
    pub map_preview: serde_json::Value,
    pub distance_range_metres: Vec<i32>,
    pub duration_range_seconds: Vec<i32>,
    pub difficulty: String,
    pub route_ids: Vec<Uuid>,
}

/// Insertable row for route summaries.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = route_summaries)]
pub(crate) struct NewRouteSummaryRow {
    pub id: Uuid,
    pub route_id: Uuid,
    pub category_id: Uuid,
    pub theme_id: Uuid,
    pub slug: Option<String>,
    pub localizations: serde_json::Value,
    pub hero_image: serde_json::Value,
    pub distance_metres: i32,
    pub duration_seconds: i32,
    pub rating: f32,
    pub badge_ids: Vec<Uuid>,
    pub difficulty: String,
    pub interest_theme_ids: Vec<Uuid>,
}

/// Insertable row for trending highlights.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = trending_route_highlights)]
pub(crate) struct NewTrendingRouteHighlightRow {
    pub id: Uuid,
    pub route_summary_id: Uuid,
    pub trend_delta: String,
    pub subtitle_localizations: serde_json::Value,
}

/// Insertable row for community picks.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = community_picks)]
pub(crate) struct NewCommunityPickRow {
    pub id: Uuid,
    pub route_summary_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub localizations: serde_json::Value,
    pub curator_display_name: String,
    pub curator_avatar: serde_json::Value,
    pub rating: f32,
    pub distance_metres: i32,
    pub duration_seconds: i32,
    pub saves: i32,
}

/// Insertable row for tag descriptors.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = tags)]
pub(crate) struct NewTagRow {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: String,
    pub localizations: serde_json::Value,
}

/// Insertable row for badge descriptors.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = badges)]
pub(crate) struct NewBadgeRow {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: String,
    pub localizations: serde_json::Value,
}

/// Insertable row for safety toggle descriptors.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = safety_toggles)]
pub(crate) struct NewSafetyToggleRow {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: String,
    pub localizations: serde_json::Value,
}

/// Insertable row for safety preset descriptors.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = safety_presets)]
pub(crate) struct NewSafetyPresetRow {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: String,
    pub localizations: serde_json::Value,
    pub safety_toggle_ids: Vec<Uuid>,
}

/// Insertable row for interest themes.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = interest_themes)]
pub(crate) struct NewInterestThemeRow {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
}
