//! Port for user preferences persistence.
//!
//! The [`UserPreferencesRepository`] trait defines the contract for storing and
//! retrieving user preferences. Adapters implement this trait to provide
//! durable storage (e.g., PostgreSQL) with support for optimistic concurrency
//! via revision checks.

use async_trait::async_trait;

use crate::domain::{UserId, UserPreferences};

use super::define_port_error;

define_port_error! {
    /// Errors raised by user preferences repository adapters.
    pub enum UserPreferencesRepositoryError {
        /// Repository connection could not be established.
        Connection { message: String } =>
            "preferences repository connection failed: {message}",
        /// Query or mutation failed during execution.
        Query { message: String } =>
            "preferences repository query failed: {message}",
        /// Optimistic concurrency check failed.
        RevisionMismatch { expected: u32, actual: u32 } =>
            "revision mismatch: expected {expected}, found {actual}",
    }
}

/// Port for user preferences storage and retrieval.
///
/// Implementations provide durable storage for user preferences, supporting
/// optimistic concurrency via revision checks. The repository follows a
/// read-modify-write pattern where updates must specify the expected revision.
///
/// # Revision Semantics
///
/// - New preferences start at revision 1.
/// - Each successful update increments the revision.
/// - Updates that specify `expected_revision` will fail with
///   [`UserPreferencesRepositoryError::RevisionMismatch`] if the current
///   revision doesn't match.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait UserPreferencesRepository: Send + Sync {
    /// Fetch preferences for a user.
    ///
    /// Returns `None` if no preferences have been saved yet for this user.
    /// Callers should initialise default preferences when `None` is returned.
    async fn find_by_user_id(
        &self,
        user_id: &UserId,
    ) -> Result<Option<UserPreferences>, UserPreferencesRepositoryError>;

    /// Save preferences with optimistic concurrency check.
    ///
    /// # Revision Check
    ///
    /// - If `expected_revision` is `None`, this is treated as an insert for
    ///   new preferences (or upsert if the database supports it).
    /// - If `expected_revision` is `Some(n)`, the update will only succeed if
    ///   the current revision equals `n`. Otherwise,
    ///   [`UserPreferencesRepositoryError::RevisionMismatch`] is returned.
    ///
    /// # Revision Increment
    ///
    /// The caller is responsible for setting `preferences.revision` to the
    /// new value before calling this method. The repository does not auto-
    /// increment the revision.
    async fn save(
        &self,
        preferences: &UserPreferences,
        expected_revision: Option<u32>,
    ) -> Result<(), UserPreferencesRepositoryError>;
}

/// Fixture implementation for testing without a real database.
///
/// This implementation always returns `None` for lookups and discards saved
/// preferences. Use it in unit tests where preferences behaviour is not under
/// test.
#[derive(Debug, Default, Clone, Copy)]
pub struct FixtureUserPreferencesRepository;

#[async_trait]
impl UserPreferencesRepository for FixtureUserPreferencesRepository {
    async fn find_by_user_id(
        &self,
        _user_id: &UserId,
    ) -> Result<Option<UserPreferences>, UserPreferencesRepositoryError> {
        Ok(None)
    }

    async fn save(
        &self,
        _preferences: &UserPreferences,
        _expected_revision: Option<u32>,
    ) -> Result<(), UserPreferencesRepositoryError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::UnitSystem;
    use chrono::Utc;
    use rstest::rstest;
    use uuid::Uuid;

    #[tokio::test]
    async fn fixture_repository_lookup_returns_none() {
        let repo = FixtureUserPreferencesRepository;
        let user_id = UserId::random();

        let result = repo
            .find_by_user_id(&user_id)
            .await
            .expect("fixture lookup should succeed");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn fixture_repository_accepts_save_operations() {
        let repo = FixtureUserPreferencesRepository;
        let prefs = UserPreferences {
            user_id: UserId::random(),
            interest_theme_ids: vec![Uuid::new_v4()],
            safety_toggle_ids: vec![],
            unit_system: UnitSystem::Metric,
            revision: 1,
            updated_at: Utc::now(),
        };

        repo.save(&prefs, None)
            .await
            .expect("fixture save should accept preferences");
    }

    #[tokio::test]
    async fn fixture_repository_accepts_save_with_expected_revision() {
        let repo = FixtureUserPreferencesRepository;
        let prefs = UserPreferences {
            user_id: UserId::random(),
            interest_theme_ids: vec![],
            safety_toggle_ids: vec![],
            unit_system: UnitSystem::Imperial,
            revision: 2,
            updated_at: Utc::now(),
        };

        repo.save(&prefs, Some(1))
            .await
            .expect("fixture save should accept expected revision");
    }

    #[rstest]
    fn revision_mismatch_error_formats_correctly() {
        let error = UserPreferencesRepositoryError::revision_mismatch(2_u32, 5_u32);
        let message = error.to_string();

        assert!(message.contains("expected 2"));
        assert!(message.contains("found 5"));
    }
}
