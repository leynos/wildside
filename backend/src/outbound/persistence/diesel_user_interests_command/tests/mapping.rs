//! Regression coverage for steady-state interests command behaviour.

use std::sync::Arc;

use rstest::rstest;

use super::super::*;
use super::support::*;
use crate::domain::ErrorCode;

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

#[tokio::test]
async fn set_interests_returns_internal_error_when_revision_bump_overflows() {
    let user_id = user_id();
    let repository = Arc::new(StubUserPreferencesRepository::with_preferences(
        UserPreferences::builder(user_id.clone())
            .interest_theme_ids(vec![uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa6")])
            .safety_toggle_ids(vec![uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa8")])
            .unit_system(UnitSystem::Metric)
            .revision(u32::MAX)
            .build(),
    ));
    let command = DieselUserInterestsCommand::new(repository.clone());

    let err = command
        .set_interests(
            &user_id,
            vec![interest_theme_id("3fa85f64-5717-4562-b3fc-2c963f66afa7")],
        )
        .await
        .expect_err("overflowing revisions should not wrap");

    assert_eq!(err.code(), ErrorCode::InternalError);
    assert!(
        err.message()
            .contains("preferences revision overflow prevents interest update")
    );
    assert!(repository.last_save_call().is_none());
}

#[tokio::test]
async fn set_interests_maps_exhausted_revision_mismatches_to_internal_error() {
    let repository = Arc::new(StubUserPreferencesRepository::default());
    repository.set_save_failures([
        StubFailure::RevisionMismatch {
            expected: 0,
            actual: 1,
        },
        StubFailure::RevisionMismatch {
            expected: 0,
            actual: 1,
        },
        StubFailure::RevisionMismatch {
            expected: 0,
            actual: 1,
        },
    ]);
    let command = DieselUserInterestsCommand::new(repository);

    let err = command
        .set_interests(
            &user_id(),
            vec![interest_theme_id("3fa85f64-5717-4562-b3fc-2c963f66afa6")],
        )
        .await
        .expect_err("exhausted revision mismatches should map to domain errors");

    assert_eq!(err.code(), ErrorCode::InternalError);
    assert!(err.message().contains("revision mismatch"));
}
