//! Route annotations: notes and progress tracking.
//!
//! This module defines domain types for user-generated annotations on routes,
//! including `RouteNote` for textual notes and `RouteProgress` for tracking
//! visited stops. Both types support optimistic concurrency via revision numbers.

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::UserId;

mod query_impl;
pub mod service;
mod service_ops;
#[cfg(test)]
mod tests;

/// A user's note on a route or specific POI.
///
/// Notes use optimistic concurrency via the `revision` field. When updating a
/// note, clients must provide the current revision; mismatches result in
/// conflict errors, ensuring concurrent edits are detected.
///
/// # Examples
///
/// ```
/// # use backend::domain::{RouteNote, UserId};
/// # use chrono::Utc;
/// # use uuid::Uuid;
/// let note = RouteNote {
///     id: Uuid::new_v4(),
///     route_id: Uuid::new_v4(),
///     poi_id: None,
///     user_id: UserId::random(),
///     body: "Beautiful viewpoint!".to_owned(),
///     created_at: Utc::now(),
///     updated_at: Utc::now(),
///     revision: 1,
/// };
///
/// assert_eq!(note.revision, 1);
/// assert!(note.poi_id.is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct RouteNote {
    /// Unique identifier (client-generated UUID).
    pub id: Uuid,
    /// The route this note belongs to.
    pub route_id: Uuid,
    /// Optional POI this note is attached to.
    pub poi_id: Option<Uuid>,
    /// The user who created the note.
    pub user_id: UserId,
    /// Note content.
    pub body: String,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
    /// Revision number for optimistic concurrency.
    pub revision: u32,
}

/// Content parameters for creating a route note.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct RouteNoteContent {
    /// The note body text.
    pub body: String,
    /// Optional POI this note is attached to.
    pub poi_id: Option<Uuid>,
}

impl RouteNoteContent {
    /// Create content with a body and optional POI attachment.
    pub fn new(body: impl Into<String>) -> Self {
        Self {
            body: body.into(),
            poi_id: None,
        }
    }

    /// Attach the note to a specific POI.
    pub fn with_poi(mut self, poi_id: Uuid) -> Self {
        self.poi_id = Some(poi_id);
        self
    }
}

impl RouteNote {
    /// Create a new note with initial revision.
    ///
    /// Sets both `created_at` and `updated_at` to the current time and
    /// initialises `revision` to 1.
    pub fn new(id: Uuid, route_id: Uuid, user_id: UserId, content: RouteNoteContent) -> Self {
        let now = Utc::now();
        Self {
            id,
            route_id,
            poi_id: content.poi_id,
            user_id,
            body: content.body,
            created_at: now,
            updated_at: now,
            revision: 1,
        }
    }

    /// Create a builder for constructing notes incrementally.
    pub fn builder(id: Uuid, route_id: Uuid, user_id: UserId) -> RouteNoteBuilder {
        RouteNoteBuilder::new(id, route_id, user_id)
    }
}

/// Builder for constructing [`RouteNote`] incrementally.
#[derive(Debug, Clone)]
pub struct RouteNoteBuilder {
    id: Uuid,
    route_id: Uuid,
    poi_id: Option<Uuid>,
    user_id: UserId,
    body: String,
    created_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
    revision: u32,
}

impl RouteNoteBuilder {
    /// Create a new builder with required fields.
    pub fn new(id: Uuid, route_id: Uuid, user_id: UserId) -> Self {
        Self {
            id,
            route_id,
            poi_id: None,
            user_id,
            body: String::new(),
            created_at: None,
            updated_at: None,
            revision: 1,
        }
    }

    /// Attach the note to a specific POI.
    pub fn poi_id(mut self, poi_id: Uuid) -> Self {
        self.poi_id = Some(poi_id);
        self
    }

    /// Set the note body.
    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into();
        self
    }

    /// Set the creation timestamp.
    pub fn created_at(mut self, ts: DateTime<Utc>) -> Self {
        self.created_at = Some(ts);
        self
    }

    /// Set the update timestamp.
    pub fn updated_at(mut self, ts: DateTime<Utc>) -> Self {
        self.updated_at = Some(ts);
        self
    }

    /// Set the revision number.
    pub fn revision(mut self, rev: u32) -> Self {
        self.revision = rev;
        self
    }

    /// Build the final [`RouteNote`] instance.
    pub fn build(self) -> RouteNote {
        let now = Utc::now();
        RouteNote {
            id: self.id,
            route_id: self.route_id,
            poi_id: self.poi_id,
            user_id: self.user_id,
            body: self.body,
            created_at: self.created_at.unwrap_or(now),
            updated_at: self.updated_at.unwrap_or(now),
            revision: self.revision,
        }
    }
}

/// Progress tracking for a route walk.
///
/// Progress uses optimistic concurrency via the `revision` field. When updating
/// progress, clients must provide the current revision to detect concurrent
/// modifications.
///
/// # Examples
///
/// ```
/// # use backend::domain::{RouteProgress, UserId};
/// # use chrono::Utc;
/// # use uuid::Uuid;
/// let stop_id = Uuid::new_v4();
/// let progress = RouteProgress::builder(Uuid::new_v4(), UserId::random())
///     .visited_stop_ids(vec![stop_id])
///     .build();
///
/// assert_eq!(progress.visited_stop_ids().len(), 1);
/// assert!(progress.has_visited(&stop_id));
/// ```
#[derive(Debug, Clone)]
pub struct RouteProgress {
    /// The route being tracked.
    pub route_id: Uuid,
    /// The user tracking progress.
    pub user_id: UserId,
    /// IDs of stops that have been visited (ordered for serialization).
    visited_stop_ids: Vec<Uuid>,
    /// HashSet for O(1) membership checks (derived from visited_stop_ids).
    visited_stop_set: HashSet<Uuid>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
    /// Revision number for optimistic concurrency.
    pub revision: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
struct RouteProgressDto {
    route_id: Uuid,
    user_id: UserId,
    visited_stop_ids: Vec<Uuid>,
    updated_at: DateTime<Utc>,
    revision: u32,
}

impl PartialEq for RouteProgress {
    fn eq(&self, other: &Self) -> bool {
        self.route_id == other.route_id
            && self.user_id == other.user_id
            && self.visited_stop_ids == other.visited_stop_ids
            && self.updated_at == other.updated_at
            && self.revision == other.revision
    }
}

impl Eq for RouteProgress {}

impl From<&RouteProgress> for RouteProgressDto {
    fn from(value: &RouteProgress) -> Self {
        Self {
            route_id: value.route_id,
            user_id: value.user_id.clone(),
            visited_stop_ids: value.visited_stop_ids.clone(),
            updated_at: value.updated_at,
            revision: value.revision,
        }
    }
}

impl From<RouteProgressDto> for RouteProgress {
    fn from(value: RouteProgressDto) -> Self {
        RouteProgress::builder(value.route_id, value.user_id)
            .visited_stop_ids(value.visited_stop_ids)
            .updated_at(value.updated_at)
            .revision(value.revision)
            .build()
    }
}

impl Serialize for RouteProgress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        RouteProgressDto::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RouteProgress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        RouteProgressDto::deserialize(deserializer).map(RouteProgress::from)
    }
}

/// Aggregated annotations for a route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteAnnotations {
    /// The route these annotations belong to.
    pub route_id: Uuid,
    /// Notes created by the user for this route.
    pub notes: Vec<RouteNote>,
    /// Progress state for this route.
    pub progress: Option<RouteProgress>,
}

impl RouteProgress {
    /// Create new progress tracking with no visited stops.
    ///
    /// Initialises with an empty `visited_stop_ids` list, revision 1, and the
    /// current timestamp.
    pub fn new(route_id: Uuid, user_id: UserId) -> Self {
        RouteProgressBuilder::new(route_id, user_id).build()
    }

    /// Create a builder for constructing progress incrementally.
    pub fn builder(route_id: Uuid, user_id: UserId) -> RouteProgressBuilder {
        RouteProgressBuilder::new(route_id, user_id)
    }

    /// Get the visited stop IDs.
    pub fn visited_stop_ids(&self) -> &[Uuid] {
        &self.visited_stop_ids
    }

    /// Check if a stop has been visited.
    ///
    /// Uses an internal `HashSet` for O(1) lookup performance.
    pub fn has_visited(&self, stop_id: &Uuid) -> bool {
        self.visited_stop_set.contains(stop_id)
    }

    /// Calculate the completion percentage given the total number of stops.
    ///
    /// Returns 0.0 if `total_stops` is 0.
    pub fn completion_percent(&self, total_stops: usize) -> f64 {
        if total_stops == 0 {
            return 0.0;
        }
        (self.visited_stop_ids.len() as f64 / total_stops as f64) * 100.0
    }
}

/// Builder for constructing [`RouteProgress`] incrementally.
#[derive(Debug, Clone)]
pub struct RouteProgressBuilder {
    route_id: Uuid,
    user_id: UserId,
    visited_stop_ids: Vec<Uuid>,
    updated_at: Option<DateTime<Utc>>,
    revision: u32,
}

impl RouteProgressBuilder {
    /// Create a new builder with required fields.
    pub fn new(route_id: Uuid, user_id: UserId) -> Self {
        Self {
            route_id,
            user_id,
            visited_stop_ids: Vec::new(),
            updated_at: None,
            revision: 1,
        }
    }

    /// Set the visited stop IDs.
    pub fn visited_stop_ids(mut self, ids: Vec<Uuid>) -> Self {
        self.visited_stop_ids = ids;
        self
    }

    /// Set the update timestamp.
    pub fn updated_at(mut self, ts: DateTime<Utc>) -> Self {
        self.updated_at = Some(ts);
        self
    }

    /// Set the revision number.
    pub fn revision(mut self, rev: u32) -> Self {
        self.revision = rev;
        self
    }

    /// Build the final [`RouteProgress`] instance.
    pub fn build(self) -> RouteProgress {
        let visited_stop_set: HashSet<Uuid> = self.visited_stop_ids.iter().copied().collect();
        RouteProgress {
            route_id: self.route_id,
            user_id: self.user_id,
            visited_stop_ids: self.visited_stop_ids,
            visited_stop_set,
            updated_at: self.updated_at.unwrap_or_else(Utc::now),
            revision: self.revision,
        }
    }
}
