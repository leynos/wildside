//! Driving port for route annotation operations.
//!
//! The [`RouteAnnotationsCommand`] trait defines the inbound contract for
//! managing route notes and progress tracking. HTTP handlers and other adapters
//! call this port to create, update, and delete annotations, with support for
//! idempotency and optimistic concurrency.

use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::{Error, IdempotencyKey, RouteNote, RouteProgress, UserId};

/// Request to upsert a route note.
#[derive(Debug, Clone)]
pub struct UpsertNoteRequest {
    /// The note's unique identifier (client-generated).
    pub note_id: Uuid,
    /// The route this note belongs to.
    pub route_id: Uuid,
    /// Optional POI this note is attached to.
    pub poi_id: Option<Uuid>,
    /// The user creating or updating the note.
    pub user_id: UserId,
    /// Note content.
    pub body: String,
    /// Expected revision for optimistic concurrency.
    ///
    /// - `None` for new notes.
    /// - `Some(n)` for updates, ensuring the current revision is `n`.
    pub expected_revision: Option<u32>,
    /// Optional idempotency key for safe retries.
    pub idempotency_key: Option<IdempotencyKey>,
}

/// Response from upserting a note.
#[derive(Debug, Clone)]
pub struct UpsertNoteResponse {
    /// The created or updated note.
    pub note: RouteNote,
    /// Whether this response was replayed from a previous idempotent request.
    pub replayed: bool,
}

/// Request to delete a route note.
#[derive(Debug, Clone)]
pub struct DeleteNoteRequest {
    /// The note's unique identifier.
    pub note_id: Uuid,
    /// The user requesting deletion (for authorisation).
    pub user_id: UserId,
    /// Optional idempotency key for safe retries.
    pub idempotency_key: Option<IdempotencyKey>,
}

/// Response from deleting a note.
#[derive(Debug, Clone)]
pub struct DeleteNoteResponse {
    /// Whether the note was actually deleted (false if it didn't exist).
    pub deleted: bool,
    /// Whether this response was replayed from a previous idempotent request.
    pub replayed: bool,
}

/// Request to update route progress.
#[derive(Debug, Clone)]
pub struct UpdateProgressRequest {
    /// The route being tracked.
    pub route_id: Uuid,
    /// The user tracking progress.
    pub user_id: UserId,
    /// IDs of stops that have been visited.
    pub visited_stop_ids: Vec<Uuid>,
    /// Expected revision for optimistic concurrency.
    ///
    /// - `None` for first-time progress.
    /// - `Some(n)` for updates, ensuring the current revision is `n`.
    pub expected_revision: Option<u32>,
    /// Optional idempotency key for safe retries.
    pub idempotency_key: Option<IdempotencyKey>,
}

/// Response from updating progress.
#[derive(Debug, Clone)]
pub struct UpdateProgressResponse {
    /// The updated progress record.
    pub progress: RouteProgress,
    /// Whether this response was replayed from a previous idempotent request.
    pub replayed: bool,
}

/// Driving port for route annotation operations.
///
/// This port is consumed by inbound adapters (e.g., HTTP handlers) to manage
/// route notes and progress. Implementations coordinate between the annotation
/// repository and idempotency repository to provide safe, retryable operations.
///
/// # Idempotency
///
/// When an `idempotency_key` is provided, the implementation should:
/// 1. Check if a response for this key already exists.
/// 2. If so, return the cached response with `replayed: true`.
/// 3. If not, perform the operation and cache the response.
///
/// # Optimistic Concurrency
///
/// When `expected_revision` is provided, the operation should fail with a
/// conflict error if the current revision doesn't match.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait RouteAnnotationsCommand: Send + Sync {
    /// Upsert a route note with idempotency and revision check.
    ///
    /// Creates a new note or updates an existing one. For updates, the
    /// `expected_revision` must match the current note's revision.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The revision check fails (conflict).
    /// - The idempotency key was used with a different payload (conflict).
    /// - The referenced route does not exist.
    /// - A database or connection error occurs.
    async fn upsert_note(&self, request: UpsertNoteRequest) -> Result<UpsertNoteResponse, Error>;

    /// Delete a route note with idempotency.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The user is not authorised to delete the note.
    /// - The idempotency key was used with a different payload (conflict).
    /// - A database or connection error occurs.
    async fn delete_note(&self, request: DeleteNoteRequest) -> Result<DeleteNoteResponse, Error>;

    /// Update route progress with idempotency and revision check.
    ///
    /// Creates new progress or updates existing progress. For updates, the
    /// `expected_revision` must match the current progress revision.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The revision check fails (conflict).
    /// - The idempotency key was used with a different payload (conflict).
    /// - The referenced route does not exist.
    /// - A database or connection error occurs.
    async fn update_progress(
        &self,
        request: UpdateProgressRequest,
    ) -> Result<UpdateProgressResponse, Error>;
}

/// Fixture implementation for testing.
///
/// Always returns default values without persisting anything.
#[derive(Debug, Default, Clone, Copy)]
pub struct FixtureRouteAnnotationsCommand;

#[async_trait]
impl RouteAnnotationsCommand for FixtureRouteAnnotationsCommand {
    async fn upsert_note(&self, request: UpsertNoteRequest) -> Result<UpsertNoteResponse, Error> {
        let now = chrono::Utc::now();
        let note = RouteNote {
            id: request.note_id,
            route_id: request.route_id,
            poi_id: request.poi_id,
            user_id: request.user_id,
            body: request.body,
            created_at: now,
            updated_at: now,
            revision: request.expected_revision.map_or(1, |r| r + 1),
        };

        Ok(UpsertNoteResponse {
            note,
            replayed: false,
        })
    }

    async fn delete_note(&self, _request: DeleteNoteRequest) -> Result<DeleteNoteResponse, Error> {
        Ok(DeleteNoteResponse {
            deleted: false,
            replayed: false,
        })
    }

    async fn update_progress(
        &self,
        request: UpdateProgressRequest,
    ) -> Result<UpdateProgressResponse, Error> {
        let progress = RouteProgress {
            route_id: request.route_id,
            user_id: request.user_id,
            visited_stop_ids: request.visited_stop_ids,
            updated_at: chrono::Utc::now(),
            revision: request.expected_revision.map_or(1, |r| r + 1),
        };

        Ok(UpdateProgressResponse {
            progress,
            replayed: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fixture_command_upserts_note() {
        let command = FixtureRouteAnnotationsCommand;
        let request = UpsertNoteRequest {
            note_id: Uuid::new_v4(),
            route_id: Uuid::new_v4(),
            poi_id: None,
            user_id: UserId::random(),
            body: "Test note".to_owned(),
            expected_revision: None,
            idempotency_key: None,
        };

        let response = command.upsert_note(request).await.expect("should succeed");

        assert!(!response.replayed);
        assert_eq!(response.note.revision, 1);
        assert_eq!(response.note.body, "Test note");
    }

    #[tokio::test]
    async fn fixture_command_increments_note_revision() {
        let command = FixtureRouteAnnotationsCommand;
        let request = UpsertNoteRequest {
            note_id: Uuid::new_v4(),
            route_id: Uuid::new_v4(),
            poi_id: Some(Uuid::new_v4()),
            user_id: UserId::random(),
            body: "Updated note".to_owned(),
            expected_revision: Some(2),
            idempotency_key: None,
        };

        let response = command.upsert_note(request).await.expect("should succeed");

        assert_eq!(response.note.revision, 3);
    }

    #[tokio::test]
    async fn fixture_command_deletes_note() {
        let command = FixtureRouteAnnotationsCommand;
        let request = DeleteNoteRequest {
            note_id: Uuid::new_v4(),
            user_id: UserId::random(),
            idempotency_key: None,
        };

        let response = command.delete_note(request).await.expect("should succeed");

        assert!(!response.deleted);
        assert!(!response.replayed);
    }

    #[tokio::test]
    async fn fixture_command_updates_progress() {
        let command = FixtureRouteAnnotationsCommand;
        let stop_ids = vec![Uuid::new_v4(), Uuid::new_v4()];
        let request = UpdateProgressRequest {
            route_id: Uuid::new_v4(),
            user_id: UserId::random(),
            visited_stop_ids: stop_ids.clone(),
            expected_revision: None,
            idempotency_key: None,
        };

        let response = command
            .update_progress(request)
            .await
            .expect("should succeed");

        assert!(!response.replayed);
        assert_eq!(response.progress.revision, 1);
        assert_eq!(response.progress.visited_stop_ids, stop_ids);
    }

    #[tokio::test]
    async fn fixture_command_increments_progress_revision() {
        let command = FixtureRouteAnnotationsCommand;
        let request = UpdateProgressRequest {
            route_id: Uuid::new_v4(),
            user_id: UserId::random(),
            visited_stop_ids: vec![],
            expected_revision: Some(5),
            idempotency_key: None,
        };

        let response = command
            .update_progress(request)
            .await
            .expect("should succeed");

        assert_eq!(response.progress.revision, 6);
    }
}
