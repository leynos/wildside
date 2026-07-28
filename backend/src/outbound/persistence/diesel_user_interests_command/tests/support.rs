//! Shared test support for interests command regression coverage.

use std::sync::Mutex;

use async_trait::async_trait;
use uuid::Uuid;

use super::super::*;
use crate::domain::UserId;
use crate::domain::ports::UpdateUserInterestsRequest;

#[derive(Clone, Copy)]
pub(super) enum StubFailure {
    Connection,
    Query,
    RevisionMismatch { expected: u32, actual: u32 },
    MissingForUpdate { expected: u32 },
    ConcurrentWriteConflict,
}

impl StubFailure {
    fn to_error(self) -> UserPreferencesRepositoryError {
        match self {
            Self::Connection => UserPreferencesRepositoryError::connection("database unavailable"),
            Self::Query => UserPreferencesRepositoryError::query("database query failed"),
            Self::RevisionMismatch { expected, actual } => {
                UserPreferencesRepositoryError::revision_mismatch(expected, actual)
            }
            Self::MissingForUpdate { expected } => {
                UserPreferencesRepositoryError::missing_for_update(expected)
            }
            Self::ConcurrentWriteConflict => {
                UserPreferencesRepositoryError::concurrent_write_conflict()
            }
        }
    }
}

#[derive(Default)]
pub(super) struct StubUserPreferencesRepository {
    stored_preferences: Mutex<Option<UserPreferences>>,
    find_failure: Mutex<Option<StubFailure>>,
    save_failures: Mutex<Vec<StubFailure>>,
    save_call_count: Mutex<usize>,
    last_save: Mutex<Option<(UserPreferences, Option<u32>)>>,
}

impl StubUserPreferencesRepository {
    pub(super) fn with_preferences(stored_preferences: UserPreferences) -> Self {
        Self {
            stored_preferences: Mutex::new(Some(stored_preferences)),
            ..Self::default()
        }
    }

    pub(super) fn set_find_failure(&self, failure: StubFailure) -> Result<(), String> {
        *self
            .find_failure
            .lock()
            .map_err(|error| format!("failed to lock find failure: {error}"))? = Some(failure);
        Ok(())
    }

    pub(super) fn set_save_failure(&self, failure: StubFailure) -> Result<(), String> {
        *self
            .save_failures
            .lock()
            .map_err(|error| format!("failed to lock save failures: {error}"))? = vec![failure];
        Ok(())
    }

    pub(super) fn last_save_call(&self) -> Result<Option<(UserPreferences, Option<u32>)>, String> {
        self.last_save
            .lock()
            .map(|last_save| last_save.clone())
            .map_err(|error| format!("failed to lock last save: {error}"))
    }

    pub(super) fn save_call_count(&self) -> Result<usize, String> {
        self.save_call_count
            .lock()
            .map(|count| *count)
            .map_err(|error| format!("failed to lock save call count: {error}"))
    }
}

#[async_trait]
impl UserPreferencesRepository for StubUserPreferencesRepository {
    async fn find_by_user_id(
        &self,
        user_id: &UserId,
    ) -> Result<Option<UserPreferences>, UserPreferencesRepositoryError> {
        let find_failure = *self
            .find_failure
            .lock()
            .map_err(|_| UserPreferencesRepositoryError::query("find failure lock poisoned"))?;
        if let Some(failure) = find_failure {
            return Err(failure.to_error());
        }

        Ok(self
            .stored_preferences
            .lock()
            .map_err(|_| UserPreferencesRepositoryError::query("stored preferences lock poisoned"))?
            .as_ref()
            .filter(|preferences| preferences.user_id == *user_id)
            .cloned())
    }

    async fn save(
        &self,
        preferences: &UserPreferences,
        expected_revision: Option<u32>,
    ) -> Result<(), UserPreferencesRepositoryError> {
        *self.save_call_count.lock().map_err(|_| {
            UserPreferencesRepositoryError::query("save call count lock poisoned")
        })? += 1;

        let failure = {
            let mut failures = self.save_failures.lock().map_err(|_| {
                UserPreferencesRepositoryError::query("save failures lock poisoned")
            })?;
            if failures.is_empty() {
                None
            } else {
                Some(failures.remove(0))
            }
        };

        if let Some(failure) = failure {
            return Err(failure.to_error());
        }

        let stored_revision = self
            .stored_preferences
            .lock()
            .map_err(|_| UserPreferencesRepositoryError::query("stored preferences lock poisoned"))?
            .as_ref()
            .map(|stored_preferences| stored_preferences.revision);

        match (stored_revision, expected_revision) {
            (None, None) | (Some(_), Some(_)) => {}
            (Some(actual), None) => {
                return Err(UserPreferencesRepositoryError::revision_mismatch(
                    0_u32, actual,
                ));
            }
            (None, Some(expected)) => {
                return Err(UserPreferencesRepositoryError::missing_for_update(expected));
            }
        }

        *self.stored_preferences.lock().map_err(|_| {
            UserPreferencesRepositoryError::query("stored preferences lock poisoned")
        })? = Some(preferences.clone());
        *self
            .last_save
            .lock()
            .map_err(|_| UserPreferencesRepositoryError::query("last save lock poisoned"))? =
            Some((preferences.clone(), expected_revision));
        Ok(())
    }
}

pub(super) fn user_id() -> Result<UserId, crate::domain::user::UserValidationError> {
    UserId::new("11111111-1111-1111-1111-111111111111")
}

pub(super) fn interest_theme_id(
    value: &str,
) -> Result<InterestThemeId, crate::domain::interest_theme::InterestThemeIdValidationError> {
    InterestThemeId::new(value)
}

pub(super) fn uuid_id(value: &str) -> Result<Uuid, uuid::Error> {
    Uuid::parse_str(value)
}

pub(super) fn request(
    user_id: UserId,
    interest_theme_ids: Vec<InterestThemeId>,
    expected_revision: Option<u32>,
) -> UpdateUserInterestsRequest {
    UpdateUserInterestsRequest {
        user_id,
        interest_theme_ids,
        expected_revision,
    }
}
