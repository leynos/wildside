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

struct PreferencesUpdate {
    preferences: UserPreferences,
    expected_revision: Option<u32>,
}

impl DieselUserInterestsCommand {
    /// Create a new interests command adapter backed by a user preferences repository.
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
            let preferences = UserPreferences::builder(user_id.clone())
                .interest_theme_ids(
                    interest_theme_ids
                        .iter()
                        .map(|interest_theme_id| *interest_theme_id.as_uuid())
                        .collect(),
                )
                .safety_toggle_ids(existing.safety_toggle_ids)
                .unit_system(existing.unit_system)
                .revision(expected_revision + 1)
                .build();
            PreferencesUpdate {
                preferences,
                expected_revision: Some(expected_revision),
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
        let existing_preferences = self
            .preferences_repository
            .find_by_user_id(user_id)
            .await
            .map_err(map_preferences_persistence_error)?;

        let update = build_preferences_for_interest_update(
            user_id,
            existing_preferences,
            &interest_theme_ids,
        );

        self.preferences_repository
            .save(&update.preferences, update.expected_revision)
            .await
            .map_err(map_preferences_persistence_error)?;

        Ok(UserInterests::new(user_id.clone(), interest_theme_ids))
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for interests persistence wiring and error mapping.
    use std::sync::Mutex;

    use super::*;
    use crate::domain::ErrorCode;
    use rstest::rstest;
    use uuid::Uuid;

    #[derive(Clone, Copy)]
    enum StubFailure {
        Connection,
        Query,
        RevisionMismatch { expected: u32, actual: u32 },
    }

    impl StubFailure {
        fn to_error(self) -> UserPreferencesRepositoryError {
            match self {
                Self::Connection => {
                    UserPreferencesRepositoryError::connection("database unavailable")
                }
                Self::Query => UserPreferencesRepositoryError::query("database query failed"),
                Self::RevisionMismatch { expected, actual } => {
                    UserPreferencesRepositoryError::revision_mismatch(expected, actual)
                }
            }
        }
    }

    #[derive(Default)]
    struct StubUserPreferencesRepository {
        stored_preferences: Mutex<Option<UserPreferences>>,
        find_failure: Mutex<Option<StubFailure>>,
        save_failure: Mutex<Option<StubFailure>>,
        last_save: Mutex<Option<(UserPreferences, Option<u32>)>>,
    }

    impl StubUserPreferencesRepository {
        fn with_preferences(stored_preferences: UserPreferences) -> Self {
            Self {
                stored_preferences: Mutex::new(Some(stored_preferences)),
                ..Self::default()
            }
        }

        fn set_find_failure(&self, failure: StubFailure) {
            *self.find_failure.lock().expect("find failure lock") = Some(failure);
        }

        fn set_save_failure(&self, failure: StubFailure) {
            *self.save_failure.lock().expect("save failure lock") = Some(failure);
        }

        fn last_save_call(&self) -> Option<(UserPreferences, Option<u32>)> {
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
            if let Some(failure) = *self.save_failure.lock().expect("save failure lock") {
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

    fn user_id() -> UserId {
        UserId::new("11111111-1111-1111-1111-111111111111").expect("valid user id")
    }

    fn interest_theme_id(value: &str) -> InterestThemeId {
        InterestThemeId::new(value).expect("valid interest theme id")
    }

    fn uuid_id(value: &str) -> Uuid {
        Uuid::parse_str(value).expect("valid uuid")
    }

    #[tokio::test]
    async fn set_interests_inserts_defaults_when_preferences_are_missing() {
        let repository = Arc::new(StubUserPreferencesRepository::default());
        let command = DieselUserInterestsCommand::new(repository.clone());
        let user_id = user_id();
        let interest_theme_ids = vec![
            interest_theme_id("3fa85f64-5717-4562-b3fc-2c963f66afa6"),
            interest_theme_id("3fa85f64-5717-4562-b3fc-2c963f66afa7"),
        ];

        let interests = command
            .set_interests(&user_id, interest_theme_ids.clone())
            .await
            .expect("set interests should succeed");

        assert_eq!(interests.user_id(), &user_id);
        assert_eq!(
            interests.interest_theme_ids(),
            interest_theme_ids.as_slice()
        );

        let (saved_preferences, expected_revision) = repository
            .last_save_call()
            .expect("save call should be recorded");
        assert_eq!(expected_revision, None);
        assert_eq!(saved_preferences.user_id, user_id);
        assert_eq!(
            saved_preferences.interest_theme_ids,
            vec![
                uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa6"),
                uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa7"),
            ]
        );
        assert!(saved_preferences.safety_toggle_ids.is_empty());
        assert_eq!(saved_preferences.unit_system, UnitSystem::Metric);
        assert_eq!(saved_preferences.revision, 1);
    }

    #[tokio::test]
    async fn set_interests_updates_existing_preferences_with_revision_bump() {
        let user_id = user_id();
        let existing_preferences = UserPreferences::builder(user_id.clone())
            .interest_theme_ids(vec![uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa6")])
            .safety_toggle_ids(vec![uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa8")])
            .unit_system(UnitSystem::Imperial)
            .revision(7)
            .build();
        let repository = Arc::new(StubUserPreferencesRepository::with_preferences(
            existing_preferences,
        ));
        let command = DieselUserInterestsCommand::new(repository.clone());
        let next_interest_ids = vec![
            interest_theme_id("3fa85f64-5717-4562-b3fc-2c963f66afa7"),
            interest_theme_id("3fa85f64-5717-4562-b3fc-2c963f66afa9"),
        ];

        let interests = command
            .set_interests(&user_id, next_interest_ids.clone())
            .await
            .expect("set interests should succeed");

        assert_eq!(interests.user_id(), &user_id);
        assert_eq!(interests.interest_theme_ids(), next_interest_ids.as_slice());

        let (saved_preferences, expected_revision) = repository
            .last_save_call()
            .expect("save call should be recorded");
        assert_eq!(expected_revision, Some(7));
        assert_eq!(
            saved_preferences.interest_theme_ids,
            vec![
                uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa7"),
                uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa9"),
            ]
        );
        assert_eq!(
            saved_preferences.safety_toggle_ids,
            vec![uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa8")]
        );
        assert_eq!(saved_preferences.unit_system, UnitSystem::Imperial);
        assert_eq!(saved_preferences.revision, 8);
    }

    #[rstest]
    #[case(StubFailure::Connection, ErrorCode::ServiceUnavailable)]
    #[case(StubFailure::Query, ErrorCode::InternalError)]
    #[tokio::test]
    async fn set_interests_maps_find_failures(
        #[case] failure: StubFailure,
        #[case] expected_code: ErrorCode,
    ) {
        let repository = Arc::new(StubUserPreferencesRepository::default());
        repository.set_find_failure(failure);
        let command = DieselUserInterestsCommand::new(repository);

        let err = command
            .set_interests(
                &user_id(),
                vec![interest_theme_id("3fa85f64-5717-4562-b3fc-2c963f66afa6")],
            )
            .await
            .expect_err("find failures should map to domain errors");

        assert_eq!(err.code(), expected_code);
    }

    #[rstest]
    #[case(StubFailure::Connection, ErrorCode::ServiceUnavailable)]
    #[case(StubFailure::Query, ErrorCode::InternalError)]
    #[case(
        StubFailure::RevisionMismatch {
            expected: 3,
            actual: 4,
        },
        ErrorCode::InternalError
    )]
    #[tokio::test]
    async fn set_interests_maps_save_failures(
        #[case] failure: StubFailure,
        #[case] expected_code: ErrorCode,
    ) {
        let repository = Arc::new(StubUserPreferencesRepository::default());
        repository.set_save_failure(failure);
        let command = DieselUserInterestsCommand::new(repository);

        let err = command
            .set_interests(
                &user_id(),
                vec![interest_theme_id("3fa85f64-5717-4562-b3fc-2c963f66afa6")],
            )
            .await
            .expect_err("save failures should map to domain errors");

        assert_eq!(err.code(), expected_code);
    }
}
