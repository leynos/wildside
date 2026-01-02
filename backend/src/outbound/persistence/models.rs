//! Internal Diesel row structs for database operations.
//!
//! These types are implementation details of the persistence layer and must
//! never be exposed to the domain. They exist solely to satisfy Diesel's
//! type requirements for queries and mutations.
//!
//! # Conversion
//!
//! Repository implementations are responsible for converting between these
//! internal models and domain types. This keeps Diesel dependencies confined
//! to the outbound adapter layer.

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use uuid::Uuid;

use super::schema::{
    idempotency_keys, route_notes, route_progress, routes, user_preferences, users,
};

/// Row struct for reading from the users table.
///
/// Maps directly to a SELECT result with all columns. Timestamp fields are
/// included to match the database schema even when not currently exposed
/// through the domain model.
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
///
/// Only includes columns that must be provided at insert time; timestamps
/// default to `NOW()` via the database schema.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = users)]
pub(crate) struct NewUserRow<'a> {
    pub id: Uuid,
    pub display_name: &'a str,
}

/// Changeset struct for updating existing user records.
///
/// Used with `ON CONFLICT DO UPDATE` for upsert operations.
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
    pub user_id: Uuid,
    pub request_id: Uuid,
    pub plan_snapshot: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Insertable struct for creating new route records.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = routes)]
pub(crate) struct NewRouteRow<'a> {
    pub id: Uuid,
    pub user_id: Uuid,
    pub request_id: Uuid,
    pub plan_snapshot: &'a serde_json::Value,
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
