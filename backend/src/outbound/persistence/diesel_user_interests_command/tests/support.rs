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

    pub(super) fn set_find_failure(&self, failure: StubFailure) {
        *self.find_failure.lock().expect("find failure lock") = Some(failure);
    }

    pub(super) fn set_save_failure(&self, failure: StubFailure) {
        *self.save_failures.lock().expect("save failure lock") = vec![failure];
    }

    pub(super) fn last_save_call(&self) -> Option<(UserPreferences, Option<u32>)> {
        self.last_save.lock().expect("last save lock").clone()
    }

    pub(super) fn save_call_count(&self) -> usize {
        *self.save_call_count.lock().expect("save call count lock")
    }
}

#[async_trait]
impl UserPreferencesRepository for StubUserPreferencesRepository {
    async fn find_by_user_id(
        &self,
        user_id: &UserId,
    ) -> Result<Option<UserPreferences>, UserPreferencesRepositoryError> {
        if let Some(failure) = *self.find_failure.lock().expect("find failure lock") {
            return Err(failure.to_error());
        }

        Ok(self
            .stored_preferences
            .lock()
            .expect("stored preferences lock")
            .as_ref()
            .filter(|preferences| preferences.user_id == *user_id)
            .cloned())
    }

    async fn save(
        &self,
        preferences: &UserPreferences,
        expected_revision: Option<u32>,
    ) -> Result<(), UserPreferencesRepositoryError> {
        *self.save_call_count.lock().expect("save call count lock") += 1;

        let failure = {
            let mut failures = self.save_failures.lock().expect("save failure lock");
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
            .expect("stored preferences lock")
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

        *self
            .stored_preferences
            .lock()
            .expect("stored preferences lock") = Some(preferences.clone());
        *self.last_save.lock().expect("last save lock") =
            Some((preferences.clone(), expected_revision));
        Ok(())
    }
}

pub(super) fn user_id() -> UserId {
    UserId::new("11111111-1111-1111-1111-111111111111").expect("valid user id")
}

pub(super) fn interest_theme_id(value: &str) -> InterestThemeId {
    InterestThemeId::new(value).expect("valid interest theme id")
}

pub(super) fn uuid_id(value: &str) -> Uuid {
    Uuid::parse_str(value).expect("valid uuid")
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
