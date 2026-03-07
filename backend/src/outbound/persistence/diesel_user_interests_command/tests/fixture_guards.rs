//! Regression coverage for retry-fixture invariants and revision validation.

use super::super::*;
use super::support::*;

fn other_user_id() -> UserId {
    UserId::new("22222222-2222-2222-2222-222222222222").expect("valid user id")
}

fn preferences_for(user_id: UserId, revision: u32) -> UserPreferences {
    UserPreferences::builder(user_id)
        .interest_theme_ids(vec![uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa6")])
        .safety_toggle_ids(Vec::new())
        .unit_system(UnitSystem::Metric)
        .revision(revision)
        .build()
}

#[test]
#[should_panic(
    expected = "InsertRaceRetryRepository competing_preferences must belong to the provided user"
)]
fn insert_race_retry_repository_rejects_mixed_user_fixtures() {
    let _ = InsertRaceRetryRepository::new(user_id(), preferences_for(other_user_id(), 1));
}

#[test]
#[should_panic(expected = "StaleUpdateRetryRepository preferences must belong to the same user")]
fn stale_update_retry_repository_rejects_mixed_user_fixtures() {
    let _ = StaleUpdateRetryRepository::new(
        preferences_for(user_id(), 2),
        preferences_for(other_user_id(), 3),
    );
}

#[tokio::test]
async fn insert_race_retry_repository_rejects_stale_retry_revision() {
    let repository = InsertRaceRetryRepository::new(user_id(), preferences_for(user_id(), 1));
    let retry_preferences = preferences_for(user_id(), 2);

    repository
        .save(&retry_preferences, None)
        .await
        .expect_err("first save should force an insert-race mismatch");

    let err = repository
        .save(&retry_preferences, Some(0))
        .await
        .expect_err("retry should validate the stored revision");

    match err {
        UserPreferencesRepositoryError::RevisionMismatch { expected, actual } => {
            assert_eq!(expected, 0);
            assert_eq!(actual, 1);
        }
        other => panic!("expected revision mismatch, got {other:?}"),
    }
}

#[tokio::test]
async fn stale_update_retry_repository_rejects_stale_retry_revision() {
    let repository = StaleUpdateRetryRepository::new(
        preferences_for(user_id(), 2),
        preferences_for(user_id(), 3),
    );
    let retry_preferences = preferences_for(user_id(), 4);

    repository
        .save(&retry_preferences, Some(2))
        .await
        .expect_err("first save should force a stale-update mismatch");

    let err = repository
        .save(&retry_preferences, Some(2))
        .await
        .expect_err("retry should validate the stored revision");

    match err {
        UserPreferencesRepositoryError::RevisionMismatch { expected, actual } => {
            assert_eq!(expected, 2);
            assert_eq!(actual, 3);
        }
        other => panic!("expected revision mismatch, got {other:?}"),
    }
}
