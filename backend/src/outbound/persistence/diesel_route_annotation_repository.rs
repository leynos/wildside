//! PostgreSQL-backed `RouteAnnotationRepository` implementation using Diesel ORM.
//!
//! This adapter implements the domain's `RouteAnnotationRepository` port, providing
//! durable storage for route notes and progress with optimistic concurrency support
//! via revision checks.

use async_trait::async_trait;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::domain::ports::{RouteAnnotationRepository, RouteAnnotationRepositoryError};
use crate::domain::{RouteNote, RouteProgress, UserId};
use crate::query_and_disambiguate;
use crate::query_optional;
use crate::query_vec;
use crate::save_with_revision;

use super::diesel_helpers::{
    HasRevision, cast_revision, cast_revision_for_db, map_diesel_error, map_pool_error,
};
use super::models::{
    NewRouteNoteRow, NewRouteProgressRow, RouteNoteRow, RouteNoteUpdate, RouteProgressRow,
    RouteProgressUpdate,
};
use super::pool::DbPool;
use super::schema::{route_notes, route_progress};

/// Diesel-backed implementation of the `RouteAnnotationRepository` port.
///
/// Provides PostgreSQL persistence for route notes and progress, supporting
/// optimistic concurrency via revision checks.
#[derive(Clone)]
pub struct DieselRouteAnnotationRepository {
    pool: DbPool,
}

impl DieselRouteAnnotationRepository {
    /// Create a new repository with the given connection pool.
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

/// Convert a database row to a domain RouteNote.
fn row_to_note(row: RouteNoteRow) -> RouteNote {
    RouteNote {
        id: row.id,
        route_id: row.route_id,
        poi_id: row.poi_id,
        user_id: UserId::from_uuid(row.user_id),
        body: row.body,
        created_at: row.created_at,
        updated_at: row.updated_at,
        revision: cast_revision(row.revision),
    }
}

/// Convert a database row to a domain RouteProgress.
fn row_to_progress(row: RouteProgressRow) -> RouteProgress {
    RouteProgress::builder(row.route_id, UserId::from_uuid(row.user_id))
        .visited_stop_ids(row.visited_stop_ids)
        .updated_at(row.updated_at)
        .revision(cast_revision(row.revision))
        .build()
}

impl HasRevision for RouteNoteRow {
    fn revision(&self) -> u32 {
        cast_revision(self.revision)
    }
}

impl HasRevision for RouteProgressRow {
    fn revision(&self) -> u32 {
        cast_revision(self.revision)
    }
}

/// Handle failed note update by checking if it's a revision mismatch or missing note.
async fn handle_note_update_failure<C>(
    conn: &mut C,
    note_id: Uuid,
    expected_revision: u32,
) -> RouteAnnotationRepositoryError
where
    C: diesel_async::AsyncConnection<Backend = diesel::pg::Pg> + Send,
{
    query_and_disambiguate!(
        conn,
        route_notes::table,
        route_notes::id.eq(note_id),
        RouteNoteRow,
        expected_revision,
        "note not found"
    )
}

/// Handle failed progress update by checking if it's a revision mismatch or missing progress.
async fn handle_progress_update_failure<C>(
    conn: &mut C,
    route_id: Uuid,
    user_id: Uuid,
    expected_revision: u32,
) -> RouteAnnotationRepositoryError
where
    C: diesel_async::AsyncConnection<Backend = diesel::pg::Pg> + Send,
{
    query_and_disambiguate!(
        conn,
        route_progress::table,
        route_progress::route_id
            .eq(route_id)
            .and(route_progress::user_id.eq(user_id)),
        RouteProgressRow,
        expected_revision,
        "progress not found"
    )
}

#[async_trait]
impl RouteAnnotationRepository for DieselRouteAnnotationRepository {
    // --- Notes ---

    async fn find_note_by_id(
        &self,
        note_id: &Uuid,
    ) -> Result<Option<RouteNote>, RouteAnnotationRepositoryError> {
        query_optional!(
            self,
            route_notes::table,
            route_notes::id.eq(note_id),
            RouteNoteRow,
            row_to_note
        )
    }

    async fn find_notes_by_route_and_user(
        &self,
        route_id: &Uuid,
        user_id: &UserId,
    ) -> Result<Vec<RouteNote>, RouteAnnotationRepositoryError> {
        query_vec!(
            self,
            route_notes::table,
            route_notes::route_id
                .eq(route_id)
                .and(route_notes::user_id.eq(user_id.as_uuid())),
            route_notes::created_at.asc(),
            RouteNoteRow,
            row_to_note
        )
    }

    async fn save_note(
        &self,
        note: &RouteNote,
        expected_revision: Option<u32>,
    ) -> Result<(), RouteAnnotationRepositoryError> {
        save_with_revision!(
            self,
            expected_revision,
            insert: {
                table: route_notes::table,
                new_row: NewRouteNoteRow {
                    id: note.id,
                    route_id: note.route_id,
                    poi_id: note.poi_id,
                    user_id: *note.user_id.as_uuid(),
                    body: &note.body,
                    revision: cast_revision_for_db(note.revision),
                }
            },
            update(expected): {
                table: route_notes::table,
                filter: route_notes::id
                    .eq(note.id)
                    .and(route_notes::revision.eq(cast_revision_for_db(expected))),
                changeset: RouteNoteUpdate {
                    poi_id: note.poi_id,
                    body: &note.body,
                    revision: cast_revision_for_db(note.revision),
                    updated_at: note.updated_at,
                },
                on_zero_rows: |conn, expected| handle_note_update_failure(conn, note.id, expected)
            }
        )
    }

    async fn delete_note(&self, note_id: &Uuid) -> Result<bool, RouteAnnotationRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let deleted = diesel::delete(route_notes::table.filter(route_notes::id.eq(note_id)))
            .execute(&mut conn)
            .await
            .map_err(map_diesel_error)?;

        Ok(deleted > 0)
    }

    // --- Progress ---

    async fn find_progress(
        &self,
        route_id: &Uuid,
        user_id: &UserId,
    ) -> Result<Option<RouteProgress>, RouteAnnotationRepositoryError> {
        query_optional!(
            self,
            route_progress::table,
            route_progress::route_id
                .eq(route_id)
                .and(route_progress::user_id.eq(user_id.as_uuid())),
            RouteProgressRow,
            row_to_progress
        )
    }

    async fn save_progress(
        &self,
        progress: &RouteProgress,
        expected_revision: Option<u32>,
    ) -> Result<(), RouteAnnotationRepositoryError> {
        save_with_revision!(
            self,
            expected_revision,
            insert: {
                table: route_progress::table,
                new_row: NewRouteProgressRow {
                    route_id: progress.route_id,
                    user_id: *progress.user_id.as_uuid(),
                    visited_stop_ids: progress.visited_stop_ids(),
                    revision: cast_revision_for_db(progress.revision),
                }
            },
            update(expected): {
                table: route_progress::table,
                filter: route_progress::route_id
                    .eq(progress.route_id)
                    .and(route_progress::user_id.eq(progress.user_id.as_uuid()))
                    .and(route_progress::revision.eq(cast_revision_for_db(expected))),
                changeset: RouteProgressUpdate {
                    visited_stop_ids: progress.visited_stop_ids(),
                    revision: cast_revision_for_db(progress.revision),
                    updated_at: progress.updated_at,
                },
                on_zero_rows: |conn, expected| handle_progress_update_failure(
                    conn,
                    progress.route_id,
                    *progress.user_id.as_uuid(),
                    expected
                )
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outbound::persistence::pool::PoolError;
    use rstest::rstest;

    #[rstest]
    fn pool_error_maps_to_connection_error() {
        let pool_err = PoolError::checkout("connection refused");
        let repo_err = map_pool_error(pool_err);

        assert!(matches!(
            repo_err,
            RouteAnnotationRepositoryError::Connection { .. }
        ));
        assert!(repo_err.to_string().contains("connection refused"));
    }

    #[rstest]
    fn diesel_error_maps_to_query_error() {
        let diesel_err = diesel::result::Error::NotFound;
        let repo_err = map_diesel_error(diesel_err);

        assert!(matches!(
            repo_err,
            RouteAnnotationRepositoryError::Query { .. }
        ));
        assert!(repo_err.to_string().contains("record not found"));
    }

    #[rstest]
    fn row_to_note_converts_correctly() {
        use chrono::Utc;

        let now = Utc::now();
        let row = RouteNoteRow {
            id: Uuid::new_v4(),
            route_id: Uuid::new_v4(),
            poi_id: Some(Uuid::new_v4()),
            user_id: Uuid::new_v4(),
            body: "Test note".to_string(),
            revision: 5,
            created_at: now,
            updated_at: now,
        };

        let note = row_to_note(row.clone());

        assert_eq!(note.id, row.id);
        assert_eq!(note.route_id, row.route_id);
        assert_eq!(note.poi_id, row.poi_id);
        assert_eq!(note.body, "Test note");
        assert_eq!(note.revision, 5);
    }

    #[rstest]
    fn row_to_progress_converts_correctly() {
        use chrono::Utc;

        let stops = vec![Uuid::new_v4(), Uuid::new_v4()];
        let row = RouteProgressRow {
            route_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            visited_stop_ids: stops.clone(),
            revision: 3,
            updated_at: Utc::now(),
        };

        let progress = row_to_progress(row.clone());

        assert_eq!(progress.route_id, row.route_id);
        assert_eq!(progress.visited_stop_ids(), stops.as_slice());
        assert_eq!(progress.revision, 3);
    }
}
