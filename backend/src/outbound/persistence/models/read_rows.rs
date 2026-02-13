//! Diesel queryable rows used by catalogue and descriptor read adapters.

use diesel::prelude::*;
use uuid::Uuid;

use crate::outbound::persistence::schema::{
    badges, community_picks, interest_themes, route_categories, route_collections, route_summaries,
    safety_presets, safety_toggles, tags, themes, trending_route_highlights,
};

// ---------------------------------------------------------------------------
// Catalogue read rows
// ---------------------------------------------------------------------------

/// Queryable row for route categories.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = route_categories)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub(crate) struct RouteCategoryRow {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: String,
    pub localizations: serde_json::Value,
    pub route_count: i32,
}

/// Queryable row for themes.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = themes)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub(crate) struct ThemeRow {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: String,
    pub localizations: serde_json::Value,
    pub image: serde_json::Value,
    pub walk_count: i32,
    pub distance_range_metres: Vec<i32>,
    pub rating: f32,
}

/// Queryable row for route collections.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = route_collections)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub(crate) struct RouteCollectionRow {
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

/// Queryable row for route summaries.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = route_summaries)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub(crate) struct RouteSummaryRow {
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

/// Queryable row for trending route highlights.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = trending_route_highlights)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub(crate) struct TrendingRouteHighlightRow {
    pub id: Uuid,
    pub route_summary_id: Uuid,
    pub trend_delta: String,
    pub subtitle_localizations: serde_json::Value,
}

/// Queryable row for community picks.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = community_picks)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub(crate) struct CommunityPickRow {
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

// ---------------------------------------------------------------------------
// Descriptor read rows
// ---------------------------------------------------------------------------

/// Queryable row for tag descriptors.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = tags)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub(crate) struct TagRow {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: String,
    pub localizations: serde_json::Value,
}

/// Queryable row for badge descriptors.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = badges)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub(crate) struct BadgeRow {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: String,
    pub localizations: serde_json::Value,
}

/// Queryable row for safety toggle descriptors.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = safety_toggles)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub(crate) struct SafetyToggleRow {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: String,
    pub localizations: serde_json::Value,
}

/// Queryable row for safety preset descriptors.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = safety_presets)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub(crate) struct SafetyPresetRow {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: String,
    pub localizations: serde_json::Value,
    pub safety_toggle_ids: Vec<Uuid>,
}

/// Queryable row for interest themes.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = interest_themes)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub(crate) struct InterestThemeRow {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
}
