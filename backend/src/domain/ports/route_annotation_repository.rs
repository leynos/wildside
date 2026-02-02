//! Port for route annotation persistence (notes and progress).
//!
//! The [`RouteAnnotationRepository`] trait defines the contract for storing and
//! retrieving route annotations, including user notes and progress tracking.
//! Adapters implement this trait to provide durable storage with support for
//! optimistic concurrency via revision checks.

use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::{RouteNote, RouteProgress, UserId};

use super::define_port_error;

define_port_error! {
    /// Errors raised by route annotation repository adapters.
    pub enum RouteAnnotationRepositoryError {
        /// Repository connection could not be established.
        Connection { message: String } =>
            "annotation repository connection failed: {message}",
        /// Query or mutation failed during execution.
        Query { message: String } =>
            "annotation repository query failed: {message}",
        /// Optimistic concurrency check failed.
        RevisionMismatch { expected: u32, actual: u32 } =>
            "revision mismatch: expected {expected}, found {actual}",
        /// Referenced route does not exist.
        RouteNotFound { route_id: String } =>
            "route not found: {route_id}",
    }
}

/// Port for route annotation storage and retrieval.
///
/// Implementations provide durable storage for route notes and progress,
/// supporting optimistic concurrency via revision checks. The repository
/// follows a read-modify-write pattern where updates must specify the
/// expected revision.
///
/// # Revision Semantics
///
/// - New notes and progress records start at revision 1.
/// - Each successful update increments the revision.
/// - Updates that specify `expected_revision` will fail with
///   [`RouteAnnotationRepositoryError::RevisionMismatch`] if the current
///   revision doesn't match.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait RouteAnnotationRepository: Send + Sync {
    // --- Notes ---

    /// Fetch a note by its unique identifier.
    ///
    /// Returns `None` if no note exists with the given ID.
    async fn find_note_by_id(
        &self,
        note_id: &Uuid,
    ) -> Result<Option<RouteNote>, RouteAnnotationRepositoryError>;

    /// Fetch all notes for a route created by a specific user.
    ///
    /// Returns an empty vector if no notes exist.
    async fn find_notes_by_route_and_user(
        &self,
        route_id: &Uuid,
        user_id: &UserId,
    ) -> Result<Vec<RouteNote>, RouteAnnotationRepositoryError>;

    /// Save a note with optimistic concurrency check.
    ///
    /// # Revision Check
    ///
    /// - If `expected_revision` is `None`, this is treated as an insert for a
    ///   new note.
    /// - If `expected_revision` is `Some(n)`, the update will only succeed if
    ///   the current revision equals `n`. Otherwise,
    ///   [`RouteAnnotationRepositoryError::RevisionMismatch`] is returned.
    ///
    /// # Note
    ///
    /// The caller is responsible for setting `note.revision` to the new value
    /// before calling this method. The repository does not auto-increment the
    /// revision.
    async fn save_note(
        &self,
        note: &RouteNote,
        expected_revision: Option<u32>,
    ) -> Result<(), RouteAnnotationRepositoryError>;

    /// Delete a note by its unique identifier.
    ///
    /// Returns `Ok(true)` if the note was deleted, `Ok(false)` if it didn't
    /// exist.
    async fn delete_note(&self, note_id: &Uuid) -> Result<bool, RouteAnnotationRepositoryError>;

    // --- Progress ---

    /// Fetch progress for a route by a specific user.
    ///
    /// Returns `None` if no progress has been recorded yet.
    async fn find_progress(
        &self,
        route_id: &Uuid,
        user_id: &UserId,
    ) -> Result<Option<RouteProgress>, RouteAnnotationRepositoryError>;

    /// Save progress with optimistic concurrency check.
    ///
    /// # Revision Check
    ///
    /// - If `expected_revision` is `None`, this is treated as an insert for
    ///   new progress.
    /// - If `expected_revision` is `Some(n)`, the update will only succeed if
    ///   the current revision equals `n`. Otherwise,
    ///   [`RouteAnnotationRepositoryError::RevisionMismatch`] is returned.
    ///
    /// # Note
    ///
    /// The caller is responsible for setting `progress.revision` to the new
    /// value before calling this method. The repository does not auto-
    /// increment the revision.
    async fn save_progress(
        &self,
        progress: &RouteProgress,
        expected_revision: Option<u32>,
    ) -> Result<(), RouteAnnotationRepositoryError>;
}

/// Fixture implementation for testing without a real database.
///
/// This implementation returns empty results for lookups and discards saved
/// records. Use it in unit tests where annotation behaviour is not under test.
#[derive(Debug, Default, Clone, Copy)]
pub struct FixtureRouteAnnotationRepository;

#[async_trait]
impl RouteAnnotationRepository for FixtureRouteAnnotationRepository {
    async fn find_note_by_id(
        &self,
        _note_id: &Uuid,
    ) -> Result<Option<RouteNote>, RouteAnnotationRepositoryError> {
        Ok(None)
    }

    async fn find_notes_by_route_and_user(
        &self,
        _route_id: &Uuid,
        _user_id: &UserId,
    ) -> Result<Vec<RouteNote>, RouteAnnotationRepositoryError> {
        Ok(Vec::new())
    }

    async fn save_note(
        &self,
        _note: &RouteNote,
        _expected_revision: Option<u32>,
    ) -> Result<(), RouteAnnotationRepositoryError> {
        Ok(())
    }

    async fn delete_note(&self, _note_id: &Uuid) -> Result<bool, RouteAnnotationRepositoryError> {
        Ok(false)
    }

    async fn find_progress(
        &self,
        _route_id: &Uuid,
        _user_id: &UserId,
    ) -> Result<Option<RouteProgress>, RouteAnnotationRepositoryError> {
        Ok(None)
    }

    async fn save_progress(
        &self,
        _progress: &RouteProgress,
        _expected_revision: Option<u32>,
    ) -> Result<(), RouteAnnotationRepositoryError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for this module.
    use super::*;
    use chrono::Utc;
    use rstest::rstest;

    /// Macro for testing fixture repository methods that return empty results.
    ///
    /// Reduces duplication when testing that the fixture implementation returns
    /// None or empty collections.
    macro_rules! test_fixture_returns_empty {
        // Pattern for methods returning Option<T> - assert is_none()
        ($test_name:ident, $method:ident($($arg:expr),*) => None) => {
            #[tokio::test]
            async fn $test_name() {
                let repo = FixtureRouteAnnotationRepository;
                let result = repo
                    .$method($($arg),*)
                    .await
                    .expect("fixture lookup should succeed");
                assert!(result.is_none());
            }
        };
        // Pattern for methods returning Vec<T> - assert is_empty()
        ($test_name:ident, $method:ident($($arg:expr),*) => Empty) => {
            #[tokio::test]
            async fn $test_name() {
                let repo = FixtureRouteAnnotationRepository;
                let result = repo
                    .$method($($arg),*)
                    .await
                    .expect("fixture lookup should succeed");
                assert!(result.is_empty());
            }
        };
    }

    test_fixture_returns_empty!(
        fixture_repository_note_lookup_returns_none,
        find_note_by_id(&Uuid::new_v4()) => None
    );

    test_fixture_returns_empty!(
        fixture_repository_notes_by_route_returns_empty,
        find_notes_by_route_and_user(&Uuid::new_v4(), &UserId::random()) => Empty
    );

    #[tokio::test]
    async fn fixture_repository_accepts_save_note() {
        let repo = FixtureRouteAnnotationRepository;
        let note = RouteNote {
            id: Uuid::new_v4(),
            route_id: Uuid::new_v4(),
            poi_id: None,
            user_id: UserId::random(),
            body: "Test note".to_owned(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            revision: 1,
        };

        repo.save_note(&note, None)
            .await
            .expect("fixture save should accept note");
    }

    #[tokio::test]
    async fn fixture_repository_delete_returns_false() {
        let repo = FixtureRouteAnnotationRepository;
        let note_id = Uuid::new_v4();

        let deleted = repo
            .delete_note(&note_id)
            .await
            .expect("fixture delete should succeed");
        assert!(!deleted);
    }

    test_fixture_returns_empty!(
        fixture_repository_progress_lookup_returns_none,
        find_progress(&Uuid::new_v4(), &UserId::random()) => None
    );

    #[tokio::test]
    async fn fixture_repository_accepts_save_progress() {
        let repo = FixtureRouteAnnotationRepository;
        let progress = RouteProgress::builder(Uuid::new_v4(), UserId::random())
            .visited_stop_ids(vec![Uuid::new_v4()])
            .revision(1)
            .build();

        repo.save_progress(&progress, None)
            .await
            .expect("fixture save should accept progress");
    }

    #[rstest]
    fn revision_mismatch_error_formats_correctly() {
        let error = RouteAnnotationRepositoryError::revision_mismatch(3_u32, 7_u32);
        let message = error.to_string();

        assert!(message.contains("expected 3"));
        assert!(message.contains("found 7"));
    }

    #[rstest]
    fn route_not_found_error_formats_correctly() {
        let route_id = Uuid::new_v4();
        let error = RouteAnnotationRepositoryError::route_not_found(route_id.to_string());
        let message = error.to_string();

        assert!(message.contains(&route_id.to_string()));
    }
}
