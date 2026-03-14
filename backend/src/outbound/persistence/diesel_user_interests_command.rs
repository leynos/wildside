//! Diesel-backed `UserInterestsCommand` adapter using user preferences storage.
//!
//! The current schema stores interest theme selections on `user_preferences`.
//! This adapter updates that projection while keeping the existing HTTP
//! contract for `/api/v1/users/me/interests` stable.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;

use crate::domain::ports::{
    UpdateUserInterestsRequest, UserInterestsCommand, UserPreferencesRepository,
    UserPreferencesRepositoryError,
};
use crate::domain::{Error, InterestThemeId, UnitSystem, UserInterests, UserPreferences};

/// Diesel-backed `UserInterestsCommand` implementation.
#[derive(Clone)]
pub struct DieselUserInterestsCommand {
    preferences_repository: Arc<dyn UserPreferencesRepository>,
}

impl DieselUserInterestsCommand {
    /// Create a new interests command adapter backed by a user preferences repository.
    ///
    /// ```rust
    /// use std::sync::Arc;
    ///
    /// use async_trait::async_trait;
    /// use backend::domain::ports::{UserPreferencesRepository, UserPreferencesRepositoryError};
    /// use backend::domain::{UserId, UserPreferences};
    /// use backend::outbound::persistence::DieselUserInterestsCommand;
    ///
    /// struct StubRepository;
    ///
    /// #[async_trait]
    /// impl UserPreferencesRepository for StubRepository {
    ///     async fn find_by_user_id(
    ///         &self,
    ///         _user_id: &UserId,
    ///     ) -> Result<Option<UserPreferences>, UserPreferencesRepositoryError> {
    ///         Ok(None)
    ///     }
    ///
    ///     async fn save(
    ///         &self,
    ///         _preferences: &UserPreferences,
    ///         _expected_revision: Option<u32>,
    ///     ) -> Result<(), UserPreferencesRepositoryError> {
    ///         Ok(())
    ///     }
    /// }
    ///
    /// let repository = Arc::new(StubRepository) as Arc<dyn UserPreferencesRepository>;
    /// let _command: DieselUserInterestsCommand = DieselUserInterestsCommand::new(repository);
    /// ```
    pub fn new(preferences_repository: Arc<dyn UserPreferencesRepository>) -> Self {
        Self {
            preferences_repository,
        }
    }
}

fn map_preferences_persistence_error(error: UserPreferencesRepositoryError) -> Error {
    match error {
        UserPreferencesRepositoryError::Connection { message } => {
            Error::service_unavailable(message)
        }
        UserPreferencesRepositoryError::Query { message } => Error::internal(message),
        UserPreferencesRepositoryError::RevisionMismatch { expected, actual } => {
            revision_conflict(Some(expected), actual)
        }
        UserPreferencesRepositoryError::MissingForUpdate { expected } => {
            revision_conflict(Some(expected), 0)
        }
        UserPreferencesRepositoryError::ConcurrentWriteConflict => {
            Error::conflict("preferences changed concurrently")
                .with_details(json!({ "code": "concurrent_write_conflict" }))
        }
    }
}

fn revision_conflict(expected: Option<u32>, actual: u32) -> Error {
    Error::conflict("revision mismatch").with_details(json!({
        "code": "revision_mismatch",
        "expectedRevision": expected,
        "actualRevision": actual,
    }))
}

fn build_preferences_for_interest_update(
    request: &UpdateUserInterestsRequest,
    existing: Option<UserPreferences>,
) -> Result<UserPreferences, Error> {
    let interest_theme_ids = request
        .interest_theme_ids
        .iter()
        .map(InterestThemeId::as_uuid)
        .copied()
        .collect();

    match (existing, request.expected_revision) {
        (None, None) => Ok(UserPreferences::builder(request.user_id.clone())
            .interest_theme_ids(interest_theme_ids)
            .safety_toggle_ids(Vec::new())
            .unit_system(UnitSystem::Metric)
            .revision(1)
            .build()),
        (None, Some(expected)) => Err(revision_conflict(Some(expected), 0)),
        (Some(existing), None) => Err(revision_conflict(None, existing.revision)),
        (Some(existing), Some(expected)) => {
            if existing.revision != expected {
                return Err(revision_conflict(Some(expected), existing.revision));
            }

            let next_revision = expected.checked_add(1).ok_or_else(|| {
                Error::internal("preferences revision overflow prevents interest update")
            })?;

            Ok(UserPreferences::builder(request.user_id.clone())
                .interest_theme_ids(interest_theme_ids)
                .safety_toggle_ids(existing.safety_toggle_ids)
                .unit_system(existing.unit_system)
                .revision(next_revision)
                .build())
        }
    }
}

#[async_trait]
impl UserInterestsCommand for DieselUserInterestsCommand {
    async fn set_interests(
        &self,
        request: UpdateUserInterestsRequest,
    ) -> Result<UserInterests, Error> {
        let existing_preferences = self
            .preferences_repository
            .find_by_user_id(&request.user_id)
            .await
            .map_err(map_preferences_persistence_error)?;
        let preferences = build_preferences_for_interest_update(&request, existing_preferences)?;

        self.preferences_repository
            .save(&preferences, request.expected_revision)
            .await
            .map_err(map_preferences_persistence_error)?;

        Ok(UserInterests::new(
            request.user_id,
            request.interest_theme_ids,
            preferences.revision,
        ))
    }
}

#[cfg(test)]
mod tests;
