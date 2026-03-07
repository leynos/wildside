//! Regression coverage for concurrent interests-update retries.

use std::sync::Arc;

use super::super::*;
use super::support::*;

#[tokio::test]
async fn set_interests_retries_after_insert_race_revision_mismatch() {
    let user_id = user_id();
    let repository = Arc::new(InsertRaceRetryRepository::new(
        user_id.clone(),
        UserPreferences::builder(user_id.clone())
            .interest_theme_ids(vec![uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa9")])
            .safety_toggle_ids(Vec::new())
            .unit_system(UnitSystem::Metric)
            .revision(1)
            .build(),
    ));
    let command = DieselUserInterestsCommand::new(repository.clone());
    let interest_theme_ids = vec![interest_theme_id("3fa85f64-5717-4562-b3fc-2c963f66afa6")];

    let interests = command
        .set_interests(&user_id, interest_theme_ids.clone())
        .await
        .expect("set interests should retry and succeed");

    assert_eq!(interests.user_id(), &user_id);
    assert_eq!(
        interests.interest_theme_ids(),
        interest_theme_ids.as_slice()
    );

    let (saved_preferences, expected_revision) = repository
        .last_save_call()
        .expect("save call should be recorded");
    assert_eq!(expected_revision, Some(1));
    assert_eq!(saved_preferences.revision, 2);
    assert_eq!(
        saved_preferences.interest_theme_ids,
        vec![uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa6")]
    );
}

#[tokio::test]
async fn set_interests_retries_after_stale_update_revision_mismatch() {
    let user_id = user_id();
    let repository = Arc::new(StaleUpdateRetryRepository::new(
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
    ));
    let command = DieselUserInterestsCommand::new(repository.clone());
    let interest_theme_ids = vec![interest_theme_id("3fa85f64-5717-4562-b3fc-2c963f66afa7")];

    let interests = command
        .set_interests(&user_id, interest_theme_ids.clone())
        .await
        .expect("set interests should retry and succeed");

    assert_eq!(interests.user_id(), &user_id);
    assert_eq!(
        interests.interest_theme_ids(),
        interest_theme_ids.as_slice()
    );

    let (saved_preferences, expected_revision) = repository
        .last_save_call()
        .expect("save call should be recorded");
    assert_eq!(expected_revision, Some(3));
    assert_eq!(saved_preferences.revision, 4);
    assert_eq!(
        saved_preferences.interest_theme_ids,
        vec![uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa7")]
    );
}

#[tokio::test]
async fn set_interests_retries_after_missing_preferences_for_update() {
    let user_id = user_id();
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
    let command = DieselUserInterestsCommand::new(repository.clone());
    let interest_theme_ids = vec![interest_theme_id("3fa85f64-5717-4562-b3fc-2c963f66afa7")];

    let interests = command
        .set_interests(&user_id, interest_theme_ids.clone())
        .await
        .expect("set interests should retry missing-update races");

    assert_eq!(interests.user_id(), &user_id);
    assert_eq!(
        interests.interest_theme_ids(),
        interest_theme_ids.as_slice()
    );

    let (saved_preferences, expected_revision) = repository
        .last_save_call()
        .expect("save call should be recorded");
    assert_eq!(expected_revision, Some(4));
    assert_eq!(saved_preferences.revision, 5);
}

#[tokio::test]
async fn set_interests_retries_after_concurrent_write_conflict() {
    let user_id = user_id();
    let repository = Arc::new(StubUserPreferencesRepository::default());
    repository.set_save_failure(StubFailure::ConcurrentWriteConflict);
    let command = DieselUserInterestsCommand::new(repository.clone());
    let interest_theme_ids = vec![interest_theme_id("3fa85f64-5717-4562-b3fc-2c963f66afa6")];

    let interests = command
        .set_interests(&user_id, interest_theme_ids.clone())
        .await
        .expect("set interests should retry concurrent write conflicts");

    assert_eq!(interests.user_id(), &user_id);
    assert_eq!(
        interests.interest_theme_ids(),
        interest_theme_ids.as_slice()
    );

    let (saved_preferences, expected_revision) = repository
        .last_save_call()
        .expect("save call should be recorded");
    assert_eq!(expected_revision, None);
    assert_eq!(saved_preferences.revision, 1);
}
