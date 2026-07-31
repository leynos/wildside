//! Shared test support for interests command regression coverage.

use std::sync::{Mutex, MutexGuard};

use super::super::*;
use crate::domain::UserId;
use crate::domain::ports::UpdateUserInterestsRequest;
use async_trait::async_trait;

/// Locks state owned by one test-local repository stub.
///
/// A panic ends the owning test, so no later operation can observe a partially
/// updated stub. Recovering a poisoned guard keeps any secondary failure from
/// obscuring the assertion that caused the original panic.
fn lock_test_state<T>(state: &Mutex<T>) -> MutexGuard<'_, T> {
    match state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

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
        *lock_test_state(&self.find_failure) = Some(failure);
    }

    pub(super) fn set_save_failure(&self, failure: StubFailure) {
        *lock_test_state(&self.save_failures) = vec![failure];
    }

    pub(super) fn last_save_call(&self) -> Option<(UserPreferences, Option<u32>)> {
        lock_test_state(&self.last_save).clone()
    }

    pub(super) fn save_call_count(&self) -> usize {
        *lock_test_state(&self.save_call_count)
    }
}

#[async_trait]
impl UserPreferencesRepository for StubUserPreferencesRepository {
    async fn find_by_user_id(
        &self,
        user_id: &UserId,
    ) -> Result<Option<UserPreferences>, UserPreferencesRepositoryError> {
        if let Some(failure) = *lock_test_state(&self.find_failure) {
            return Err(failure.to_error());
        }

        Ok(lock_test_state(&self.stored_preferences)
            .as_ref()
            .filter(|preferences| preferences.user_id == *user_id)
            .cloned())
    }

    async fn save(
        &self,
        preferences: &UserPreferences,
        expected_revision: Option<u32>,
    ) -> Result<(), UserPreferencesRepositoryError> {
        *lock_test_state(&self.save_call_count) += 1;

        let failure = {
            let mut failures = lock_test_state(&self.save_failures);
            if failures.is_empty() {
                None
            } else {
                Some(failures.remove(0))
            }
        };

        if let Some(failure) = failure {
            return Err(failure.to_error());
        }

        let stored_revision = lock_test_state(&self.stored_preferences)
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

        *lock_test_state(&self.stored_preferences) = Some(preferences.clone());
        *lock_test_state(&self.last_save) = Some((preferences.clone(), expected_revision));
        Ok(())
    }
}

macro_rules! user_id {
    () => {
        $crate::domain::UserId::new("11111111-1111-1111-1111-111111111111").expect("valid user id")
    };
}

macro_rules! interest_theme_id {
    ($value:expr) => {
        $crate::domain::InterestThemeId::new($value).expect("valid interest theme id")
    };
}

macro_rules! uuid_id {
    ($value:expr) => {
        ::uuid::Uuid::parse_str($value).expect("valid uuid")
    };
}

pub(super) use interest_theme_id;
pub(super) use user_id;
pub(super) use uuid_id;

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
