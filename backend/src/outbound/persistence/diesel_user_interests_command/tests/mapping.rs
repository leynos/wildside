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
        .set_interests(request(user_id.clone(), interest_theme_ids.clone(), None))
        .await
        .expect("set interests should succeed");

    assert_eq!(interests.user_id(), &user_id);
    assert_eq!(
        interests.interest_theme_ids(),
        interest_theme_ids.as_slice()
    );
    assert_eq!(interests.revision(), 1);

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
        .set_interests(request(user_id.clone(), next_interest_ids.clone(), Some(7)))
        .await
        .expect("set interests should succeed");

    assert_eq!(interests.user_id(), &user_id);
    assert_eq!(interests.interest_theme_ids(), next_interest_ids.as_slice());
    assert_eq!(interests.revision(), 8);

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
        .set_interests(request(
            user_id(),
            vec![interest_theme_id("3fa85f64-5717-4562-b3fc-2c963f66afa6")],
            None,
        ))
        .await
        .expect_err("find failures should map to domain errors");

    assert_eq!(err.code(), expected_code);
}

#[rstest]
#[case(StubFailure::Connection, ErrorCode::ServiceUnavailable)]
#[case(StubFailure::Query, ErrorCode::InternalError)]
#[case(
    StubFailure::RevisionMismatch {
        expected: 7,
        actual: 8,
    },
    ErrorCode::Conflict
)]
#[case(StubFailure::MissingForUpdate { expected: 7 }, ErrorCode::Conflict)]
#[case(StubFailure::ConcurrentWriteConflict, ErrorCode::Conflict)]
#[tokio::test]
async fn set_interests_maps_save_failures(
    #[case] failure: StubFailure,
    #[case] expected_code: ErrorCode,
) {
    let repository = Arc::new(StubUserPreferencesRepository::default());
    repository.set_save_failure(failure);
    let command = DieselUserInterestsCommand::new(repository);

    let err = command
        .set_interests(request(
            user_id(),
            vec![interest_theme_id("3fa85f64-5717-4562-b3fc-2c963f66afa6")],
            None,
        ))
        .await
        .expect_err("save failures should map to domain errors");

    assert_eq!(err.code(), expected_code);
}

#[tokio::test]
async fn set_interests_rejects_missing_expected_revision_when_preferences_exist() {
    let user_id = user_id();
    let repository = Arc::new(StubUserPreferencesRepository::with_preferences(
        UserPreferences::builder(user_id.clone())
            .interest_theme_ids(vec![uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa6")])
            .safety_toggle_ids(vec![uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa8")])
            .unit_system(UnitSystem::Metric)
            .revision(3)
            .build(),
    ));
    let command = DieselUserInterestsCommand::new(repository.clone());

    let err = command
        .set_interests(request(
            user_id,
            vec![interest_theme_id("3fa85f64-5717-4562-b3fc-2c963f66afa7")],
            None,
        ))
        .await
        .expect_err("missing revision should be rejected");

    assert_eq!(err.code(), ErrorCode::Conflict);
    assert_eq!(err.message(), "revision mismatch");
    assert_eq!(repository.save_call_count(), 0);
    assert_eq!(
        err.details()
            .and_then(|details| details.get("expectedRevision"))
            .map(serde_json::Value::is_null),
        Some(true)
    );
    assert_eq!(
        err.details()
            .and_then(|details| details.get("actualRevision"))
            .and_then(serde_json::Value::as_u64),
        Some(3)
    );
}

#[tokio::test]
async fn set_interests_rejects_stale_expected_revision_before_save() {
    let user_id = user_id();
    let repository = Arc::new(StubUserPreferencesRepository::with_preferences(
        UserPreferences::builder(user_id.clone())
            .interest_theme_ids(vec![uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa6")])
            .safety_toggle_ids(vec![uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa8")])
            .unit_system(UnitSystem::Metric)
            .revision(4)
            .build(),
    ));
    let command = DieselUserInterestsCommand::new(repository.clone());

    let err = command
        .set_interests(request(
            user_id,
            vec![interest_theme_id("3fa85f64-5717-4562-b3fc-2c963f66afa7")],
            Some(2),
        ))
        .await
        .expect_err("stale revision should be rejected");

    assert_eq!(err.code(), ErrorCode::Conflict);
    assert_eq!(err.message(), "revision mismatch");
    assert_eq!(repository.save_call_count(), 0);
    assert_eq!(
        err.details()
            .and_then(|details| details.get("expectedRevision"))
            .and_then(serde_json::Value::as_u64),
        Some(2)
    );
    assert_eq!(
        err.details()
            .and_then(|details| details.get("actualRevision"))
            .and_then(serde_json::Value::as_u64),
        Some(4)
    );
}

#[tokio::test]
async fn set_interests_rejects_missing_preferences_for_expected_revision() {
    let repository = Arc::new(StubUserPreferencesRepository::default());
    let command = DieselUserInterestsCommand::new(repository.clone());

    let err = command
        .set_interests(request(
            user_id(),
            vec![interest_theme_id("3fa85f64-5717-4562-b3fc-2c963f66afa6")],
            Some(9),
        ))
        .await
        .expect_err("missing preferences should conflict");

    assert_eq!(err.code(), ErrorCode::Conflict);
    assert_eq!(repository.save_call_count(), 0);
    assert_eq!(
        err.details()
            .and_then(|details| details.get("actualRevision"))
            .and_then(serde_json::Value::as_u64),
        Some(0)
    );
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
        .set_interests(request(
            user_id,
            vec![interest_theme_id("3fa85f64-5717-4562-b3fc-2c963f66afa7")],
            Some(u32::MAX),
        ))
        .await
        .expect_err("overflowing revisions should not wrap");

    assert_eq!(err.code(), ErrorCode::InternalError);
    assert!(
        err.message()
            .contains("preferences revision overflow prevents interest update")
    );
    assert_eq!(repository.save_call_count(), 0);
}
