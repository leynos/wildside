//! Diesel-backed `UserInterestsCommand` adapter using user preferences storage.
//!
//! The current schema stores interest theme selections on `user_preferences`.
//! This adapter updates that projection while keeping the existing HTTP
//! contract for `/api/v1/users/me/interests` stable.

use std::sync::Arc;

use async_trait::async_trait;

use crate::domain::ports::{
    UserInterestsCommand, UserPreferencesRepository, UserPreferencesRepositoryError,
};
use crate::domain::{Error, InterestThemeId, UnitSystem, UserId, UserInterests, UserPreferences};

/// Diesel-backed `UserInterestsCommand` implementation.
#[derive(Clone)]
pub struct DieselUserInterestsCommand {
    preferences_repository: Arc<dyn UserPreferencesRepository>,
}

const MAX_CONCURRENT_WRITE_ATTEMPTS: usize = 3;

struct PreferencesUpdate {
    preferences: UserPreferences,
    expected_revision: Option<u32>,
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
            // TODO(3.5.4): replace this temporary internal-error mapping with an
            // explicit revision-conflict contract once stale-write semantics land.
            Error::internal(format!(
                "preferences revision mismatch: expected {expected}, found {actual}"
            ))
        }
    }
}

fn build_preferences_for_interest_update(
    user_id: &UserId,
    existing: Option<UserPreferences>,
    interest_theme_ids: &[InterestThemeId],
) -> PreferencesUpdate {
    match existing {
        Some(existing) => {
            let expected_revision = existing.revision;
            let (next_revision, expected_revision) = match expected_revision.checked_add(1) {
                Some(next_revision) => (next_revision, Some(expected_revision)),
                None => (expected_revision, None),
            };
            let preferences = UserPreferences::builder(user_id.clone())
                .interest_theme_ids(
                    interest_theme_ids
                        .iter()
                        .map(|interest_theme_id| *interest_theme_id.as_uuid())
                        .collect(),
                )
                .safety_toggle_ids(existing.safety_toggle_ids)
                .unit_system(existing.unit_system)
                .revision(next_revision)
                .build();
            PreferencesUpdate {
                preferences,
                expected_revision,
            }
        }
        None => PreferencesUpdate {
            preferences: UserPreferences::builder(user_id.clone())
                .interest_theme_ids(
                    interest_theme_ids
                        .iter()
                        .map(|interest_theme_id| *interest_theme_id.as_uuid())
                        .collect(),
                )
                .safety_toggle_ids(Vec::new())
                .unit_system(UnitSystem::Metric)
                .revision(1)
                .build(),
            expected_revision: None,
        },
    }
}

#[async_trait]
impl UserInterestsCommand for DieselUserInterestsCommand {
    async fn set_interests(
        &self,
        user_id: &UserId,
        interest_theme_ids: Vec<InterestThemeId>,
    ) -> Result<UserInterests, Error> {
        for attempt in 0..MAX_CONCURRENT_WRITE_ATTEMPTS {
            let existing_preferences = self
                .preferences_repository
                .find_by_user_id(user_id)
                .await
                .map_err(map_preferences_persistence_error)?;
            let had_existing_preferences = existing_preferences.is_some();

            let update = build_preferences_for_interest_update(
                user_id,
                existing_preferences,
                &interest_theme_ids,
            );
            if had_existing_preferences && update.expected_revision.is_none() {
                return Err(Error::internal(
                    "preferences revision overflow prevents interest update",
                ));
            }

            match self
                .preferences_repository
                .save(&update.preferences, update.expected_revision)
                .await
            {
                Ok(()) => return Ok(UserInterests::new(user_id.clone(), interest_theme_ids)),
                Err(UserPreferencesRepositoryError::RevisionMismatch { .. })
                    if attempt + 1 < MAX_CONCURRENT_WRITE_ATTEMPTS =>
                {
                    continue;
                }
                Err(error) => return Err(map_preferences_persistence_error(error)),
            }
        }

        Err(Error::internal(
            "interest update retry loop exited unexpectedly",
        ))
    }
}

#[cfg(test)]
mod tests;
