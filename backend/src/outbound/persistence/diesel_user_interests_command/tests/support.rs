//! Shared test support for interests command regression coverage.

use std::sync::Mutex;

use async_trait::async_trait;
use uuid::Uuid;

use super::super::*;

#[derive(Clone, Copy)]
pub(super) enum StubFailure {
    Connection,
    Query,
    RevisionMismatch { expected: u32, actual: u32 },
}

impl StubFailure {
    fn to_error(self) -> UserPreferencesRepositoryError {
        match self {
            Self::Connection => UserPreferencesRepositoryError::connection("database unavailable"),
            Self::Query => UserPreferencesRepositoryError::query("database query failed"),
            Self::RevisionMismatch { expected, actual } => {
                UserPreferencesRepositoryError::revision_mismatch(expected, actual)
            }
        }
    }
}

#[derive(Default)]
pub(super) struct StubUserPreferencesRepository {
    stored_preferences: Mutex<Option<UserPreferences>>,
    find_failure: Mutex<Option<StubFailure>>,
    save_failures: Mutex<Vec<StubFailure>>,
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

    pub(super) fn set_save_failures(&self, failures: impl IntoIterator<Item = StubFailure>) {
        *self.save_failures.lock().expect("save failure lock") = failures.into_iter().collect();
    }

    pub(super) fn last_save_call(&self) -> Option<(UserPreferences, Option<u32>)> {
        self.last_save.lock().expect("last save lock").clone()
    }
}

pub(super) struct InsertRaceRetryRepository {
    user_id: UserId,
    competing_preferences: UserPreferences,
    attempts: Mutex<usize>,
    last_save: Mutex<Option<(UserPreferences, Option<u32>)>>,
}

impl InsertRaceRetryRepository {
    pub(super) fn new(user_id: UserId, competing_preferences: UserPreferences) -> Self {
        Self {
            user_id,
            competing_preferences,
            attempts: Mutex::new(0),
            last_save: Mutex::new(None),
        }
    }

    pub(super) fn last_save_call(&self) -> Option<(UserPreferences, Option<u32>)> {
        self.last_save.lock().expect("last save lock").clone()
    }
}

pub(super) struct StaleUpdateRetryRepository {
    initial_preferences: UserPreferences,
    competing_preferences: UserPreferences,
    attempts: Mutex<usize>,
    last_save: Mutex<Option<(UserPreferences, Option<u32>)>>,
}

impl StaleUpdateRetryRepository {
    pub(super) fn new(
        initial_preferences: UserPreferences,
        competing_preferences: UserPreferences,
    ) -> Self {
        Self {
            initial_preferences,
            competing_preferences,
            attempts: Mutex::new(0),
            last_save: Mutex::new(None),
        }
    }

    pub(super) fn last_save_call(&self) -> Option<(UserPreferences, Option<u32>)> {
        self.last_save.lock().expect("last save lock").clone()
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

        *self
            .stored_preferences
            .lock()
            .expect("stored preferences lock") = Some(preferences.clone());
        *self.last_save.lock().expect("last save lock") =
            Some((preferences.clone(), expected_revision));
        Ok(())
    }
}

#[async_trait]
impl UserPreferencesRepository for InsertRaceRetryRepository {
    async fn find_by_user_id(
        &self,
        user_id: &UserId,
    ) -> Result<Option<UserPreferences>, UserPreferencesRepositoryError> {
        if *user_id != self.user_id {
            return Ok(None);
        }

        let attempts = *self.attempts.lock().expect("attempts lock");
        if attempts == 0 {
            Ok(None)
        } else {
            Ok(Some(self.competing_preferences.clone()))
        }
    }

    async fn save(
        &self,
        preferences: &UserPreferences,
        expected_revision: Option<u32>,
    ) -> Result<(), UserPreferencesRepositoryError> {
        let mut attempts = self.attempts.lock().expect("attempts lock");
        if *attempts == 0 {
            *attempts += 1;
            return Err(UserPreferencesRepositoryError::revision_mismatch(
                0_u32, 1_u32,
            ));
        }

        *self.last_save.lock().expect("last save lock") =
            Some((preferences.clone(), expected_revision));
        Ok(())
    }
}

#[async_trait]
impl UserPreferencesRepository for StaleUpdateRetryRepository {
    async fn find_by_user_id(
        &self,
        user_id: &UserId,
    ) -> Result<Option<UserPreferences>, UserPreferencesRepositoryError> {
        if user_id != &self.initial_preferences.user_id {
            return Ok(None);
        }

        let attempts = *self.attempts.lock().expect("attempts lock");
        if attempts == 0 {
            Ok(Some(self.initial_preferences.clone()))
        } else {
            Ok(Some(self.competing_preferences.clone()))
        }
    }

    async fn save(
        &self,
        preferences: &UserPreferences,
        expected_revision: Option<u32>,
    ) -> Result<(), UserPreferencesRepositoryError> {
        let mut attempts = self.attempts.lock().expect("attempts lock");
        if *attempts == 0 {
            *attempts += 1;
            return Err(UserPreferencesRepositoryError::revision_mismatch(
                2_u32, 3_u32,
            ));
        }

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
