//! Internal Diesel row structs for database operations.
//!
//! These types are implementation details of the persistence layer and must
//! never be exposed to the domain. They exist solely to satisfy Diesel's
//! type requirements for queries and mutations.

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use uuid::Uuid;

use super::schema::{
    badges, community_picks, example_data_runs, idempotency_keys, interest_themes,
    route_categories, route_collections, route_notes, route_progress, route_summaries, routes,
    safety_presets, safety_toggles, tags, themes, trending_route_highlights, user_preferences,
    users,
};

/// Row struct for reading from the users table.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub(crate) struct UserRow {
    pub id: Uuid,
    pub display_name: String,
    #[expect(dead_code, reason = "schema field for future audit trail support")]
    pub created_at: DateTime<Utc>,
    #[expect(dead_code, reason = "schema field for future audit trail support")]
    pub updated_at: DateTime<Utc>,
}

/// Insertable struct for creating new user records.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = users)]
pub(crate) struct NewUserRow<'a> {
    pub id: Uuid,
    pub display_name: &'a str,
}

/// Changeset struct for updating existing user records.
#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = users)]
pub(crate) struct UserUpdate<'a> {
    pub display_name: &'a str,
}

// ---------------------------------------------------------------------------
// Idempotency key models
// ---------------------------------------------------------------------------

/// Row struct for reading from the idempotency_keys table.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = idempotency_keys)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub(crate) struct IdempotencyKeyRow {
    pub key: Uuid,
    pub user_id: Uuid,
    pub mutation_type: String,
    pub payload_hash: Vec<u8>,
    pub response_snapshot: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// Insertable struct for creating new idempotency records.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = idempotency_keys)]
pub(crate) struct NewIdempotencyKeyRow<'a> {
    pub key: Uuid,
    pub mutation_type: &'a str,
    pub payload_hash: &'a [u8],
    pub response_snapshot: &'a serde_json::Value,
    pub user_id: Uuid,
}

// ---------------------------------------------------------------------------
// Routes models
// ---------------------------------------------------------------------------

/// Row struct for reading from the routes table.
#[expect(
    dead_code,
    reason = "will be used when DieselRouteRepository is implemented"
)]
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = routes)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub(crate) struct RouteRow {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub path: String,
    pub generation_params: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// Insertable struct for creating new route records.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = routes)]
pub(crate) struct NewRouteRow<'a> {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub path: &'a str,
    pub generation_params: &'a serde_json::Value,
}

// ---------------------------------------------------------------------------
// User preferences models
// ---------------------------------------------------------------------------

/// Row struct for reading from the user_preferences table.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = user_preferences)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub(crate) struct UserPreferencesRow {
    pub user_id: Uuid,
    pub interest_theme_ids: Vec<Uuid>,
    pub safety_toggle_ids: Vec<Uuid>,
    pub unit_system: String,
    pub revision: i32,
    pub updated_at: DateTime<Utc>,
}

/// Insertable struct for creating new user preferences records.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = user_preferences)]
pub(crate) struct NewUserPreferencesRow<'a> {
    pub user_id: Uuid,
    pub interest_theme_ids: &'a [Uuid],
    pub safety_toggle_ids: &'a [Uuid],
    pub unit_system: &'a str,
    pub revision: i32,
}

/// Changeset struct for updating user preferences.
#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = user_preferences)]
pub(crate) struct UserPreferencesUpdate<'a> {
    pub interest_theme_ids: &'a [Uuid],
    pub safety_toggle_ids: &'a [Uuid],
    pub unit_system: &'a str,
    pub revision: i32,
}

// ---------------------------------------------------------------------------
// Route notes models
// ---------------------------------------------------------------------------

/// Row struct for reading from the route_notes table.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = route_notes)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub(crate) struct RouteNoteRow {
    pub id: Uuid,
    pub route_id: Uuid,
    pub poi_id: Option<Uuid>,
    pub user_id: Uuid,
    pub body: String,
    pub revision: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Insertable struct for creating new route note records.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = route_notes)]
pub(crate) struct NewRouteNoteRow<'a> {
    pub id: Uuid,
    pub route_id: Uuid,
    pub poi_id: Option<Uuid>,
    pub user_id: Uuid,
    pub body: &'a str,
    pub revision: i32,
}

/// Changeset struct for updating route notes.
#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = route_notes)]
pub(crate) struct RouteNoteUpdate<'a> {
    pub poi_id: Option<Uuid>,
    pub body: &'a str,
    pub revision: i32,
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Route progress models
// ---------------------------------------------------------------------------

/// Row struct for reading from the route_progress table.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = route_progress)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub(crate) struct RouteProgressRow {
    pub route_id: Uuid,
    pub user_id: Uuid,
    pub visited_stop_ids: Vec<Uuid>,
    pub revision: i32,
    pub updated_at: DateTime<Utc>,
}

/// Insertable struct for creating new route progress records.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = route_progress)]
pub(crate) struct NewRouteProgressRow<'a> {
    pub route_id: Uuid,
    pub user_id: Uuid,
    pub visited_stop_ids: &'a [Uuid],
    pub revision: i32,
}

/// Changeset struct for updating route progress.
#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = route_progress)]
pub(crate) struct RouteProgressUpdate<'a> {
    pub visited_stop_ids: &'a [Uuid],
    pub revision: i32,
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Example data runs models
// ---------------------------------------------------------------------------

/// Row struct for reading from the example_data_runs table.
#[expect(
    dead_code,
    reason = "will be used when seed audit/query functionality is added"
)]
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = example_data_runs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub(crate) struct ExampleDataRunRow {
    pub seed_key: String,
    pub seeded_at: DateTime<Utc>,
    pub user_count: i32,
    pub seed: i64,
}

/// Insertable struct for recording a new example data seed run.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = example_data_runs)]
pub(crate) struct NewExampleDataRunRow<'a> {
    pub seed_key: &'a str,
    pub user_count: i32,
    pub seed: i64,
}

// ---------------------------------------------------------------------------
// Catalogue and descriptor ingestion models
// ---------------------------------------------------------------------------

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

/// Insertable row for descriptor entries.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = tags)]
pub(crate) struct NewTagRow<'a> {
    pub id: Uuid,
    pub slug: &'a str,
    pub icon_key: &'a str,
    pub localizations: &'a serde_json::Value,
}

/// Insertable row for descriptor entries.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = badges)]
pub(crate) struct NewBadgeRow<'a> {
    pub id: Uuid,
    pub slug: &'a str,
    pub icon_key: &'a str,
    pub localizations: &'a serde_json::Value,
}

/// Insertable row for descriptor entries.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = safety_toggles)]
pub(crate) struct NewSafetyToggleRow<'a> {
    pub id: Uuid,
    pub slug: &'a str,
    pub icon_key: &'a str,
    pub localizations: &'a serde_json::Value,
}

/// Insertable row for descriptor entries.
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
