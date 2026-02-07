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
pub(crate) struct NewRouteCategoryRow<'a> {
    pub id: Uuid,
    pub slug: &'a str,
    pub icon_key: &'a str,
    pub localizations: &'a serde_json::Value,
    pub route_count: i32,
}

/// Insertable row for themes.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = themes)]
pub(crate) struct NewThemeRow<'a> {
    pub id: Uuid,
    pub slug: &'a str,
    pub icon_key: &'a str,
    pub localizations: &'a serde_json::Value,
    pub image: &'a serde_json::Value,
    pub walk_count: i32,
    pub distance_range_metres: &'a [i32],
    pub rating: f32,
}

/// Insertable row for route collections.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = route_collections)]
pub(crate) struct NewRouteCollectionRow<'a> {
    pub id: Uuid,
    pub slug: &'a str,
    pub icon_key: &'a str,
    pub localizations: &'a serde_json::Value,
    pub lead_image: &'a serde_json::Value,
    pub map_preview: &'a serde_json::Value,
    pub distance_range_metres: &'a [i32],
    pub duration_range_seconds: &'a [i32],
    pub difficulty: &'a str,
    pub route_ids: &'a [Uuid],
}

/// Insertable row for route summaries.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = route_summaries)]
pub(crate) struct NewRouteSummaryRow<'a> {
    pub id: Uuid,
    pub route_id: Uuid,
    pub category_id: Uuid,
    pub theme_id: Uuid,
    pub slug: Option<&'a str>,
    pub localizations: &'a serde_json::Value,
    pub hero_image: &'a serde_json::Value,
    pub distance_metres: i32,
    pub duration_seconds: i32,
    pub rating: f32,
    pub badge_ids: &'a [Uuid],
    pub difficulty: &'a str,
    pub interest_theme_ids: &'a [Uuid],
}

/// Insertable row for trending highlights.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = trending_route_highlights)]
pub(crate) struct NewTrendingRouteHighlightRow<'a> {
    pub id: Uuid,
    pub route_summary_id: Uuid,
    pub trend_delta: &'a str,
    pub subtitle_localizations: &'a serde_json::Value,
}

/// Insertable row for community picks.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = community_picks)]
pub(crate) struct NewCommunityPickRow<'a> {
    pub id: Uuid,
    pub route_summary_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub localizations: &'a serde_json::Value,
    pub curator_display_name: &'a str,
    pub curator_avatar: &'a serde_json::Value,
    pub rating: f32,
    pub distance_metres: i32,
    pub duration_seconds: i32,
    pub saves: i32,
}

/// Insertable row for tag descriptors.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = tags)]
pub(crate) struct NewTagRow<'a> {
    pub id: Uuid,
    pub slug: &'a str,
    pub icon_key: &'a str,
    pub localizations: &'a serde_json::Value,
}

/// Insertable row for badge descriptors.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = badges)]
pub(crate) struct NewBadgeRow<'a> {
    pub id: Uuid,
    pub slug: &'a str,
    pub icon_key: &'a str,
    pub localizations: &'a serde_json::Value,
}

/// Insertable row for safety toggle descriptors.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = safety_toggles)]
pub(crate) struct NewSafetyToggleRow<'a> {
    pub id: Uuid,
    pub slug: &'a str,
    pub icon_key: &'a str,
    pub localizations: &'a serde_json::Value,
}

/// Insertable row for safety preset descriptors.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = safety_presets)]
pub(crate) struct NewSafetyPresetRow<'a> {
    pub id: Uuid,
    pub slug: &'a str,
    pub icon_key: &'a str,
    pub localizations: &'a serde_json::Value,
    pub safety_toggle_ids: &'a [Uuid],
}

/// Insertable row for interest themes.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = interest_themes)]
pub(crate) struct NewInterestThemeRow<'a> {
    pub id: Uuid,
    pub name: &'a str,
    pub description: Option<&'a str>,
}
