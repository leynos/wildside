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

struct RetrySaveTracker {
    attempts: Mutex<usize>,
    revision: Mutex<Option<u32>>,
    last_save: Mutex<Option<(UserPreferences, Option<u32>)>>,
}

impl RetrySaveTracker {
    fn new() -> Self {
        Self {
            attempts: Mutex::new(0),
            revision: Mutex::new(None),
            last_save: Mutex::new(None),
        }
    }

    fn attempt(&self) -> usize {
        *self.attempts.lock().expect("attempts lock")
    }

    fn increment_attempts(&self) {
        *self.attempts.lock().expect("attempts lock") += 1;
    }

    fn revision(&self) -> Option<u32> {
        *self.revision.lock().expect("revision lock")
    }

    fn set_revision(&self, revision: Option<u32>) {
        *self.revision.lock().expect("revision lock") = revision;
    }

    fn record(&self, preferences: &UserPreferences, expected_revision: Option<u32>) {
        self.set_revision(Some(preferences.revision));
        *self.last_save.lock().expect("last save lock") =
            Some((preferences.clone(), expected_revision));
    }

    fn last_save_call(&self) -> Option<(UserPreferences, Option<u32>)> {
        self.last_save.lock().expect("last save lock").clone()
    }
}

pub(super) struct InsertRaceRetryRepository {
    user_id: UserId,
    competing_preferences: UserPreferences,
    tracker: RetrySaveTracker,
}

impl InsertRaceRetryRepository {
    pub(super) fn new(user_id: UserId, competing_preferences: UserPreferences) -> Self {
        assert_eq!(
            competing_preferences.user_id, user_id,
            "InsertRaceRetryRepository competing_preferences must belong to the provided user"
        );

        Self {
            user_id,
            competing_preferences,
            tracker: RetrySaveTracker::new(),
        }
    }

    pub(super) fn last_save_call(&self) -> Option<(UserPreferences, Option<u32>)> {
        self.tracker.last_save_call()
    }
}

pub(super) struct StaleUpdateRetryRepository {
    initial_preferences: UserPreferences,
    competing_preferences: UserPreferences,
    tracker: RetrySaveTracker,
}

impl StaleUpdateRetryRepository {
    pub(super) fn new(
        initial_preferences: UserPreferences,
        competing_preferences: UserPreferences,
    ) -> Self {
        assert_eq!(
            competing_preferences.user_id, initial_preferences.user_id,
            "StaleUpdateRetryRepository preferences must belong to the same user"
        );

        let repository = Self {
            initial_preferences,
            competing_preferences,
            tracker: RetrySaveTracker::new(),
        };
        repository
            .tracker
            .set_revision(Some(repository.initial_preferences.revision));
        repository
    }

    pub(super) fn last_save_call(&self) -> Option<(UserPreferences, Option<u32>)> {
        self.tracker.last_save_call()
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

        let attempts = self.tracker.attempt();
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
        if self.tracker.attempt() == 0 {
            self.tracker.increment_attempts();
            self.tracker
                .set_revision(Some(self.competing_preferences.revision));
            return Err(UserPreferencesRepositoryError::revision_mismatch(
                0_u32, 1_u32,
            ));
        }

        let stored_revision = self
            .tracker
            .revision()
            .expect("insert-race tracker revision should be available after retry");
        if expected_revision != Some(stored_revision) {
            return Err(UserPreferencesRepositoryError::revision_mismatch(
                expected_revision.unwrap_or_default(),
                stored_revision,
            ));
        }

        self.tracker.record(preferences, expected_revision);
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

        let attempts = self.tracker.attempt();
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
        if self.tracker.attempt() == 0 {
            self.tracker.increment_attempts();
            self.tracker
                .set_revision(Some(self.competing_preferences.revision));
            return Err(UserPreferencesRepositoryError::revision_mismatch(
                2_u32, 3_u32,
            ));
        }

        let stored_revision = self
            .tracker
            .revision()
            .expect("stale-update tracker revision should be available after retry");
        if expected_revision != Some(stored_revision) {
            return Err(UserPreferencesRepositoryError::revision_mismatch(
                expected_revision.unwrap_or_default(),
                stored_revision,
            ));
        }

        self.tracker.record(preferences, expected_revision);
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
