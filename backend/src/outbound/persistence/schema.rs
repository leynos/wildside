//! Diesel table definitions for the PostgreSQL schema.
//!
//! These definitions must match the database migrations exactly. They are used
//! by Diesel for compile-time query validation and type-safe SQL generation.
//!
//! # Maintenance
//!
//! When migrations change the schema, this file should be regenerated or
//! manually updated to reflect those changes. The `diesel print-schema`
//! command can generate these definitions from a live database.

// -----------------------------------------------------------------------------
// idempotency_keys table
// -----------------------------------------------------------------------------
//
// Stores idempotency records for safe request retries on outbox-backed mutations.
// Supports multiple mutation types (routes, notes, progress, preferences, bundles).
//
// Columns:
//
// - key: Client-provided UUID v4 idempotency key (part of composite primary key)
// - user_id: User who made the original request (part of composite primary key)
// - mutation_type: Type of mutation (routes, notes, etc.) (part of composite PK)
// - payload_hash: SHA-256 hash of the canonicalised request payload (32 bytes)
// - response_snapshot: JSONB snapshot of the original response to replay
// - created_at: Record creation timestamp (used for TTL-based cleanup)

diesel::table! {
    idempotency_keys (key, user_id, mutation_type) {
        key -> Uuid,
        user_id -> Uuid,
        mutation_type -> Text,
        payload_hash -> Bytea,
        response_snapshot -> Jsonb,
        created_at -> Timestamptz,
    }
}

// -----------------------------------------------------------------------------
// users table
// -----------------------------------------------------------------------------
//
// User accounts table storing registered users with their display names and
// audit timestamps. Columns:
//
// - id: Primary key (UUID v4 identifier)
// - display_name: Human-readable display name (max 32 characters)
// - created_at: Record creation timestamp
// - updated_at: Last modification timestamp (auto-updated by trigger)

diesel::table! {
    users (id) {
        id -> Uuid,
        display_name -> Varchar,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

// -----------------------------------------------------------------------------
// routes table
// -----------------------------------------------------------------------------
//
// Generated route plans stored for users. This table is a prerequisite for
// route_notes and route_progress which have foreign keys to it.
//
// Columns:
//
// - id: Primary key (UUID v4 identifier)
// - user_id: User who created the route (FK to users)
// - request_id: Original route request identifier
// - plan_snapshot: JSONB snapshot of the generated route plan
// - created_at: Record creation timestamp
// - updated_at: Last modification timestamp (auto-updated by trigger)

diesel::table! {
    routes (id) {
        id -> Uuid,
        user_id -> Uuid,
        request_id -> Uuid,
        plan_snapshot -> Jsonb,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

// -----------------------------------------------------------------------------
// user_preferences table
// -----------------------------------------------------------------------------
//
// User preferences for interests, safety settings, and display options.
// Supports optimistic concurrency via the revision column.
//
// Columns:
//
// - user_id: Primary key / FK to users
// - interest_theme_ids: Array of selected interest theme UUIDs
// - safety_toggle_ids: Array of enabled safety toggle UUIDs
// - unit_system: Display unit system ('metric' or 'imperial')
// - revision: Optimistic concurrency revision number
// - updated_at: Last modification timestamp (auto-updated by trigger)

diesel::table! {
    user_preferences (user_id) {
        user_id -> Uuid,
        interest_theme_ids -> Array<Uuid>,
        safety_toggle_ids -> Array<Uuid>,
        unit_system -> Text,
        revision -> Int4,
        updated_at -> Timestamptz,
    }
}

// -----------------------------------------------------------------------------
// route_notes table
// -----------------------------------------------------------------------------
//
// User annotations attached to routes or specific POIs within routes.
// Supports optimistic concurrency via the revision column.
//
// Columns:
//
// - id: Primary key (UUID v4 identifier)
// - route_id: FK to routes table
// - poi_id: Optional POI identifier within the route
// - user_id: FK to users table
// - body: Note content text
// - revision: Optimistic concurrency revision number
// - created_at: Record creation timestamp
// - updated_at: Last modification timestamp (auto-updated by trigger)

diesel::table! {
    route_notes (id) {
        id -> Uuid,
        route_id -> Uuid,
        poi_id -> Nullable<Uuid>,
        user_id -> Uuid,
        body -> Text,
        revision -> Int4,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

// -----------------------------------------------------------------------------
// route_progress table
// -----------------------------------------------------------------------------
//
// Progress tracking for users walking routes, storing visited stop IDs.
// Supports optimistic concurrency via the revision column.
//
// Columns:
//
// - route_id: Part of composite PK, FK to routes
// - user_id: Part of composite PK, FK to users
// - visited_stop_ids: Array of visited stop UUIDs
// - revision: Optimistic concurrency revision number
// - updated_at: Last modification timestamp (auto-updated by trigger)

diesel::table! {
    route_progress (route_id, user_id) {
        route_id -> Uuid,
        user_id -> Uuid,
        visited_stop_ids -> Array<Uuid>,
        revision -> Int4,
        updated_at -> Timestamptz,
    }
}

// -----------------------------------------------------------------------------
// Foreign key relationships
// -----------------------------------------------------------------------------

diesel::joinable!(routes -> users (user_id));
diesel::joinable!(user_preferences -> users (user_id));
diesel::joinable!(route_notes -> routes (route_id));
diesel::joinable!(route_notes -> users (user_id));
diesel::joinable!(route_progress -> routes (route_id));
diesel::joinable!(route_progress -> users (user_id));

// -----------------------------------------------------------------------------
// example_data_runs table
// -----------------------------------------------------------------------------
//
// Tracks applied example data seeds to prevent duplicate seeding. Used by the
// example-data feature to ensure once-only seeding on startup.
//
// Columns:
//
// - seed_key: Primary key (seed name, e.g., "mossy-owl")
// - seeded_at: Timestamp when seeding was performed
// - user_count: Number of users created by this seed
// - seed: The RNG seed value used for deterministic generation

diesel::table! {
    example_data_runs (seed_key) {
        seed_key -> Text,
        seeded_at -> Timestamptz,
        user_count -> Int4,
        seed -> Int8,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    example_data_runs,
    idempotency_keys,
    route_notes,
    route_progress,
    routes,
    user_preferences,
    users,
);
