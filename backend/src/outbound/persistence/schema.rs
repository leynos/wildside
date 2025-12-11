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
