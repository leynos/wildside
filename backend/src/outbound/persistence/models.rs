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

use super::schema::users;

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
