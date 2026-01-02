//! PostgreSQL-backed `RouteAnnotationRepository` implementation using Diesel ORM.
//!
//! This adapter implements the domain's `RouteAnnotationRepository` port, providing
//! durable storage for route notes and progress with optimistic concurrency support
//! via revision checks.

use async_trait::async_trait;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use tracing::debug;
use uuid::Uuid;

use crate::domain::ports::{RouteAnnotationRepository, RouteAnnotationRepositoryError};
use crate::domain::{RouteNote, RouteProgress, UserId};

use super::models::{
    NewRouteNoteRow, NewRouteProgressRow, RouteNoteRow, RouteNoteUpdate, RouteProgressRow,
    RouteProgressUpdate,
};
use super::pool::{DbPool, PoolError};
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

/// Map pool errors to domain route annotation repository errors.
fn map_pool_error(error: PoolError) -> RouteAnnotationRepositoryError {
    match error {
        PoolError::Checkout { message } | PoolError::Build { message } => {
            RouteAnnotationRepositoryError::connection(message)
        }
    }
}

/// Map Diesel errors to domain route annotation repository errors.
fn map_diesel_error(error: diesel::result::Error) -> RouteAnnotationRepositoryError {
    use diesel::result::{DatabaseErrorKind, Error as DieselError};

    match &error {
        DieselError::DatabaseError(kind, info) => {
            debug!(?kind, message = info.message(), "diesel operation failed");
        }
        _ => debug!(
            error_type = %std::any::type_name_of_val(&error),
            "diesel operation failed"
        ),
    }

    match error {
        DieselError::NotFound => RouteAnnotationRepositoryError::query("record not found"),
        DieselError::QueryBuilderError(_) => {
            RouteAnnotationRepositoryError::query("database query error")
        }
        DieselError::DatabaseError(kind, info) => match kind {
            DatabaseErrorKind::ForeignKeyViolation => {
                let message = info.message();
                if message.contains("routes") {
                    RouteAnnotationRepositoryError::route_not_found("referenced route".to_string())
                } else {
                    RouteAnnotationRepositoryError::query("foreign key violation")
                }
            }
            DatabaseErrorKind::ClosedConnection => {
                RouteAnnotationRepositoryError::connection("database connection error")
            }
            _ => RouteAnnotationRepositoryError::query("database error"),
        },
        _ => RouteAnnotationRepositoryError::query("database error"),
    }
}

/// Cast database revision (i32) to domain revision (u32).
///
/// Database stores revisions as `i32` but domain uses `u32`. Revisions are
/// always non-negative in practice, enforced by database constraints.
#[expect(
    clippy::cast_sign_loss,
    reason = "revision is always non-negative in database"
)]
fn cast_revision(revision: i32) -> u32 {
    revision as u32
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
    RouteProgress {
        route_id: row.route_id,
        user_id: UserId::from_uuid(row.user_id),
        visited_stop_ids: row.visited_stop_ids,
        updated_at: row.updated_at,
        revision: cast_revision(row.revision),
    }
}

/// Trait for database rows that have a revision field.
trait HasRevision {
    /// Get the revision as a u32.
    fn revision(&self) -> u32;
}

impl HasRevision for RouteNoteRow {
    fn revision(&self) -> u32 { cast_revision(self.revision) }
}

impl HasRevision for RouteProgressRow {
    fn revision(&self) -> u32 { cast_revision(self.revision) }
}

/// Disambiguate update failure by checking if it's a revision mismatch or missing record.
///
/// Given the result of querying for the current record, returns either a revision
/// mismatch error (if the record exists with different revision) or a not-found error
/// (if the record doesn't exist). Propagates any query errors.
fn disambiguate_update_failure<R>(
    current_result: Result<Option<R>, RouteAnnotationRepositoryError>,
    expected_revision: u32,
    not_found_message: &str,
) -> RouteAnnotationRepositoryError
where
    R: HasRevision,
{
    match current_result {
        Ok(Some(record)) => RouteAnnotationRepositoryError::revision_mismatch(
            expected_revision,
            record.revision(),
        ),
        Ok(None) => RouteAnnotationRepositoryError::query(not_found_message),
        Err(e) => e,
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
    let current_result = route_notes::table
        .filter(route_notes::id.eq(note_id))
        .select(RouteNoteRow::as_select())
        .first(conn)
        .await
        .optional()
        .map_err(map_diesel_error);

    disambiguate_update_failure(current_result, expected_revision, "note not found")
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
    let current_result = route_progress::table
        .filter(
            route_progress::route_id
                .eq(route_id)
                .and(route_progress::user_id.eq(user_id)),
        )
        .select(RouteProgressRow::as_select())
        .first(conn)
        .await
        .optional()
        .map_err(map_diesel_error);

    disambiguate_update_failure(current_result, expected_revision, "progress not found")
}

/// Cast domain revision (u32) to database revision (i32).
#[expect(
    clippy::cast_possible_wrap,
    reason = "revision values are always small positive integers"
)]
fn cast_revision_for_db(revision: u32) -> i32 {
    revision as i32
}

/// Execute update with optimistic concurrency and return sentinel error if zero rows affected.
///
/// This helper executes an update operation and returns a sentinel Query error with
/// message "update affected 0 rows" if no rows were updated, allowing callers to
/// disambiguate the failure.
async fn execute_optimistic_update(
    updated_rows: usize,
) -> Result<(), RouteAnnotationRepositoryError> {
    if updated_rows == 0 {
        Err(RouteAnnotationRepositoryError::query("update affected 0 rows"))
    } else {
        Ok(())
    }
}

/// Check if an error is the sentinel "update affected 0 rows" error.
fn is_zero_rows_error(error: &RouteAnnotationRepositoryError) -> bool {
    error.to_string().contains("update affected 0 rows")
}

#[async_trait]
impl RouteAnnotationRepository for DieselRouteAnnotationRepository {
    // --- Notes ---

    async fn find_note_by_id(
        &self,
        note_id: &Uuid,
    ) -> Result<Option<RouteNote>, RouteAnnotationRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let result: Option<RouteNoteRow> = route_notes::table
            .filter(route_notes::id.eq(note_id))
            .select(RouteNoteRow::as_select())
            .first(&mut conn)
            .await
            .optional()
            .map_err(map_diesel_error)?;

        Ok(result.map(row_to_note))
    }

    async fn find_notes_by_route_and_user(
        &self,
        route_id: &Uuid,
        user_id: &UserId,
    ) -> Result<Vec<RouteNote>, RouteAnnotationRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let rows: Vec<RouteNoteRow> = route_notes::table
            .filter(
                route_notes::route_id
                    .eq(route_id)
                    .and(route_notes::user_id.eq(user_id.as_uuid())),
            )
            .select(RouteNoteRow::as_select())
            .order_by(route_notes::created_at.asc())
            .load(&mut conn)
            .await
            .map_err(map_diesel_error)?;

        Ok(rows.into_iter().map(row_to_note).collect())
    }

    async fn save_note(
        &self,
        note: &RouteNote,
        expected_revision: Option<u32>,
    ) -> Result<(), RouteAnnotationRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;
        let revision_i32 = cast_revision_for_db(note.revision);

        match expected_revision {
            None => {
                let new_row = NewRouteNoteRow {
                    id: note.id,
                    route_id: note.route_id,
                    poi_id: note.poi_id,
                    user_id: *note.user_id.as_uuid(),
                    body: &note.body,
                    revision: revision_i32,
                };
                diesel::insert_into(route_notes::table)
                    .values(&new_row)
                    .execute(&mut conn)
                    .await
                    .map(|_| ())
                    .map_err(map_diesel_error)
            }
            Some(expected) => {
                let expected_i32 = cast_revision_for_db(expected);
                let update = RouteNoteUpdate {
                    poi_id: note.poi_id,
                    body: &note.body,
                    revision: revision_i32,
                };

                let updated_rows = diesel::update(route_notes::table)
                    .filter(
                        route_notes::id
                            .eq(note.id)
                            .and(route_notes::revision.eq(expected_i32)),
                    )
                    .set(&update)
                    .execute(&mut conn)
                    .await
                    .map_err(map_diesel_error)?;

                let result = execute_optimistic_update(updated_rows).await;
                if let Err(ref e) = result
                    && is_zero_rows_error(e)
                {
                    return Err(handle_note_update_failure(&mut conn, note.id, expected).await);
                }
                result
            }
        }
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
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let result: Option<RouteProgressRow> = route_progress::table
            .filter(
                route_progress::route_id
                    .eq(route_id)
                    .and(route_progress::user_id.eq(user_id.as_uuid())),
            )
            .select(RouteProgressRow::as_select())
            .first(&mut conn)
            .await
            .optional()
            .map_err(map_diesel_error)?;

        Ok(result.map(row_to_progress))
    }

    async fn save_progress(
        &self,
        progress: &RouteProgress,
        expected_revision: Option<u32>,
    ) -> Result<(), RouteAnnotationRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;
        let revision_i32 = cast_revision_for_db(progress.revision);

        match expected_revision {
            None => {
                let new_row = NewRouteProgressRow {
                    route_id: progress.route_id,
                    user_id: *progress.user_id.as_uuid(),
                    visited_stop_ids: &progress.visited_stop_ids,
                    revision: revision_i32,
                };
                diesel::insert_into(route_progress::table)
                    .values(&new_row)
                    .execute(&mut conn)
                    .await
                    .map(|_| ())
                    .map_err(map_diesel_error)
            }
            Some(expected) => {
                let expected_i32 = cast_revision_for_db(expected);
                let update = RouteProgressUpdate {
                    visited_stop_ids: &progress.visited_stop_ids,
                    revision: revision_i32,
                };

                let updated_rows = diesel::update(route_progress::table)
                    .filter(
                        route_progress::route_id
                            .eq(progress.route_id)
                            .and(route_progress::user_id.eq(progress.user_id.as_uuid()))
                            .and(route_progress::revision.eq(expected_i32)),
                    )
                    .set(&update)
                    .execute(&mut conn)
                    .await
                    .map_err(map_diesel_error)?;

                let result = execute_optimistic_update(updated_rows).await;
                if let Err(ref e) = result
                    && is_zero_rows_error(e)
                {
                    return Err(handle_progress_update_failure(
                        &mut conn,
                        progress.route_id,
                        *progress.user_id.as_uuid(),
                        expected,
                    )
                    .await);
                }
                result
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert_eq!(progress.visited_stop_ids, stops);
        assert_eq!(progress.revision, 3);
    }
}
