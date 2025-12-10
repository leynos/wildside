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

diesel::table! {
    /// User accounts table.
    ///
    /// Stores registered users with their display names and audit timestamps.
    /// The `id` column is the primary key (UUID v4).
    users (id) {
        /// Primary key: UUID v4 identifier.
        id -> Uuid,
        /// Human-readable display name (max 32 characters).
        display_name -> Varchar,
        /// Record creation timestamp.
        created_at -> Timestamptz,
        /// Last modification timestamp (auto-updated by trigger).
        updated_at -> Timestamptz,
    }
}
