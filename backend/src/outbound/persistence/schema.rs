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
