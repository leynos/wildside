//! Regression coverage for concurrent interests-update retries.

use std::sync::Arc;

use rstest::rstest;

use super::super::*;
use super::support::*;

enum RetryScenario {
    InsertRaceRevisionMismatch,
    StaleUpdateRevisionMismatch,
    MissingPreferencesForUpdate,
    ConcurrentWriteConflict,
}

enum RetryRepositoryHarness {
    InsertRace(Arc<InsertRaceRetryRepository>),
    StaleUpdate(Arc<StaleUpdateRetryRepository>),
    Stub(Arc<StubUserPreferencesRepository>),
}

impl RetryRepositoryHarness {
    fn command_repository(&self) -> Arc<dyn UserPreferencesRepository> {
        match self {
            Self::InsertRace(repository) => repository.clone(),
            Self::StaleUpdate(repository) => repository.clone(),
            Self::Stub(repository) => repository.clone(),
        }
    }

    fn last_save_call(&self) -> Option<(UserPreferences, Option<u32>)> {
        match self {
            Self::InsertRace(repository) => repository.last_save_call(),
            Self::StaleUpdate(repository) => repository.last_save_call(),
            Self::Stub(repository) => repository.last_save_call(),
        }
    }
}

struct RetryCase {
    repository: RetryRepositoryHarness,
    user_id: UserId,
    interest_theme_ids: Vec<InterestThemeId>,
    expected_revision: Option<u32>,
    expected_saved_revision: u32,
    expected_saved_interest_ids: Vec<uuid::Uuid>,
}

fn build_retry_case(scenario: RetryScenario) -> RetryCase {
    let user_id = user_id();

    match scenario {
        RetryScenario::InsertRaceRevisionMismatch => RetryCase {
            repository: RetryRepositoryHarness::InsertRace(Arc::new(
                InsertRaceRetryRepository::new(
                    user_id.clone(),
                    UserPreferences::builder(user_id.clone())
                        .interest_theme_ids(vec![uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa9")])
                        .safety_toggle_ids(Vec::new())
                        .unit_system(UnitSystem::Metric)
                        .revision(1)
                        .build(),
                ),
            )),
            user_id,
            interest_theme_ids: vec![interest_theme_id("3fa85f64-5717-4562-b3fc-2c963f66afa6")],
            expected_revision: Some(1),
            expected_saved_revision: 2,
            expected_saved_interest_ids: vec![uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa6")],
        },
        RetryScenario::StaleUpdateRevisionMismatch => RetryCase {
            repository: RetryRepositoryHarness::StaleUpdate(Arc::new(
                StaleUpdateRetryRepository::new(
                    UserPreferences::builder(user_id.clone())
                        .interest_theme_ids(vec![uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa6")])
                        .safety_toggle_ids(vec![uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa8")])
                        .unit_system(UnitSystem::Metric)
                        .revision(2)
                        .build(),
                    UserPreferences::builder(user_id.clone())
                        .interest_theme_ids(vec![uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afaa")])
                        .safety_toggle_ids(vec![uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa8")])
                        .unit_system(UnitSystem::Metric)
                        .revision(3)
                        .build(),
                ),
            )),
            user_id,
            interest_theme_ids: vec![interest_theme_id("3fa85f64-5717-4562-b3fc-2c963f66afa7")],
            expected_revision: Some(3),
            expected_saved_revision: 4,
            expected_saved_interest_ids: vec![uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa7")],
        },
        RetryScenario::MissingPreferencesForUpdate => {
            let existing_preferences = UserPreferences::builder(user_id.clone())
                .interest_theme_ids(vec![uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa6")])
                .safety_toggle_ids(vec![uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa8")])
                .unit_system(UnitSystem::Metric)
                .revision(4)
                .build();
            let repository = Arc::new(StubUserPreferencesRepository::with_preferences(
                existing_preferences,
            ));
            repository.set_save_failure(StubFailure::MissingForUpdate { expected: 4 });

            RetryCase {
                repository: RetryRepositoryHarness::Stub(repository),
                user_id,
                interest_theme_ids: vec![interest_theme_id("3fa85f64-5717-4562-b3fc-2c963f66afa7")],
                expected_revision: Some(4),
                expected_saved_revision: 5,
                expected_saved_interest_ids: vec![uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa7")],
            }
        }
        RetryScenario::ConcurrentWriteConflict => {
            let repository = Arc::new(StubUserPreferencesRepository::default());
            repository.set_save_failure(StubFailure::ConcurrentWriteConflict);

            RetryCase {
                repository: RetryRepositoryHarness::Stub(repository),
                user_id,
                interest_theme_ids: vec![interest_theme_id("3fa85f64-5717-4562-b3fc-2c963f66afa6")],
                expected_revision: None,
                expected_saved_revision: 1,
                expected_saved_interest_ids: vec![uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa6")],
            }
        }
    }
}

fn assert_saved_result(
    repository: &RetryRepositoryHarness,
    expected_revision: Option<u32>,
    expected_saved_revision: u32,
    expected_saved_interest_ids: &[uuid::Uuid],
) {
    let (saved_preferences, actual_expected_revision) = repository
        .last_save_call()
        .expect("save call should be recorded");
    assert_eq!(actual_expected_revision, expected_revision);
    assert_eq!(saved_preferences.revision, expected_saved_revision);
    assert_eq!(
        saved_preferences.interest_theme_ids,
        expected_saved_interest_ids
    );
}

#[rstest]
#[case::insert_race(RetryScenario::InsertRaceRevisionMismatch)]
#[case::stale_update(RetryScenario::StaleUpdateRevisionMismatch)]
#[case::missing_for_update(RetryScenario::MissingPreferencesForUpdate)]
#[case::concurrent_write_conflict(RetryScenario::ConcurrentWriteConflict)]
#[tokio::test]
async fn set_interests_retries_after_retryable_repository_conflicts(
    #[case] scenario: RetryScenario,
) {
    let retry_case = build_retry_case(scenario);
    let command = DieselUserInterestsCommand::new(retry_case.repository.command_repository());

    let interests = command
        .set_interests(&retry_case.user_id, retry_case.interest_theme_ids.clone())
        .await
        .expect("set interests should retry and succeed");

    assert_eq!(interests.user_id(), &retry_case.user_id);
    assert_eq!(
        interests.interest_theme_ids(),
        retry_case.interest_theme_ids.as_slice()
    );
    assert_saved_result(
        &retry_case.repository,
        retry_case.expected_revision,
        retry_case.expected_saved_revision,
        &retry_case.expected_saved_interest_ids,
    );
}
