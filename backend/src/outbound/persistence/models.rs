//! Internal Diesel row structs for database operations.
//!
//! These types are implementation details of the persistence layer and must
//! never be exposed to the domain. They exist solely to satisfy Diesel's
//! type requirements for queries and mutations.

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use uuid::Uuid;

use super::schema::{
    example_data_runs, idempotency_keys, offline_bundles, route_notes, route_progress, routes,
    user_preferences, users, walk_sessions,
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
// Offline bundle models
// ---------------------------------------------------------------------------

/// Row struct for reading from the offline_bundles table.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = offline_bundles)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub(crate) struct OfflineBundleRow {
    pub id: Uuid,
    pub owner_user_id: Option<Uuid>,
    pub device_id: String,
    pub kind: String,
    pub route_id: Option<Uuid>,
    pub region_id: Option<String>,
    pub bounds: Vec<f64>,
    pub min_zoom: i32,
    pub max_zoom: i32,
    pub estimated_size_bytes: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub status: String,
    pub progress: f32,
}

/// Insertable struct for creating offline bundle records.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = offline_bundles)]
pub(crate) struct NewOfflineBundleRow<'a> {
    pub id: Uuid,
    pub owner_user_id: Option<Uuid>,
    pub device_id: &'a str,
    pub kind: &'a str,
    pub route_id: Option<Uuid>,
    pub region_id: Option<&'a str>,
    pub bounds: &'a [f64],
    pub min_zoom: i32,
    pub max_zoom: i32,
    pub estimated_size_bytes: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub status: &'a str,
    pub progress: f32,
}

/// Changeset struct for upserting offline bundle records.
#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = offline_bundles)]
pub(crate) struct OfflineBundleUpdate<'a> {
    pub owner_user_id: Option<Uuid>,
    pub device_id: &'a str,
    pub kind: &'a str,
    pub route_id: Option<Uuid>,
    pub region_id: Option<&'a str>,
    pub bounds: &'a [f64],
    pub min_zoom: i32,
    pub max_zoom: i32,
    pub estimated_size_bytes: i64,
    pub updated_at: DateTime<Utc>,
    pub status: &'a str,
    pub progress: f32,
}

// ---------------------------------------------------------------------------
// Walk session models
// ---------------------------------------------------------------------------

/// Row struct for reading from the walk_sessions table.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = walk_sessions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub(crate) struct WalkSessionRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub route_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub primary_stats: serde_json::Value,
    pub secondary_stats: serde_json::Value,
    pub highlighted_poi_ids: Vec<Uuid>,
    #[expect(dead_code, reason = "schema field for auditing support")]
    pub created_at: DateTime<Utc>,
    #[expect(dead_code, reason = "schema field for auditing support")]
    pub updated_at: DateTime<Utc>,
}

/// Insertable struct for creating walk session records.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = walk_sessions)]
pub(crate) struct NewWalkSessionRow<'a> {
    pub id: Uuid,
    pub user_id: Uuid,
    pub route_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub primary_stats: &'a serde_json::Value,
    pub secondary_stats: &'a serde_json::Value,
    pub highlighted_poi_ids: &'a [Uuid],
}

/// Changeset struct for upserting walk session records.
#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = walk_sessions)]
pub(crate) struct WalkSessionUpdate<'a> {
    pub user_id: Uuid,
    pub route_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub primary_stats: &'a serde_json::Value,
    pub secondary_stats: &'a serde_json::Value,
    pub highlighted_poi_ids: &'a [Uuid],
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

mod ingestion_rows;
mod read_rows;

pub(crate) use ingestion_rows::{
    NewBadgeRow, NewCommunityPickRow, NewInterestThemeRow, NewRouteCategoryRow,
    NewRouteCollectionRow, NewRouteSummaryRow, NewSafetyPresetRow, NewSafetyToggleRow, NewTagRow,
    NewThemeRow, NewTrendingRouteHighlightRow,
};
pub(crate) use read_rows::{
    BadgeRow, CommunityPickRow, InterestThemeRow, RouteCategoryRow, RouteCollectionRow,
    RouteSummaryRow, SafetyPresetRow, SafetyToggleRow, TagRow, ThemeRow, TrendingRouteHighlightRow,
};
