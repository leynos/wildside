//! Regression coverage for repository conflict handling without hidden retries.

use std::sync::Arc;

use super::super::*;
use super::support::*;

#[tokio::test]
async fn set_interests_surfaces_insert_race_as_conflict_without_retrying() {
    let repository = Arc::new(StubUserPreferencesRepository::default());
    repository.set_save_failure(StubFailure::RevisionMismatch {
        expected: 0,
        actual: 1,
    });
    let command = DieselUserInterestsCommand::new(repository.clone());

    let err = command
        .set_interests(request(
            user_id(),
            vec![interest_theme_id("3fa85f64-5717-4562-b3fc-2c963f66afa6")],
            None,
        ))
        .await
        .expect_err("insert race should surface as conflict");

    assert_eq!(repository.save_call_count(), 1);
    assert_eq!(err.code(), crate::domain::ErrorCode::Conflict);
    assert_eq!(err.message(), "revision mismatch");
    assert_eq!(
        err.details()
            .and_then(|details| details.get("expectedRevision"))
            .and_then(serde_json::Value::as_u64),
        Some(0)
    );
    assert_eq!(
        err.details()
            .and_then(|details| details.get("actualRevision"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[tokio::test]
async fn set_interests_surfaces_repository_stale_write_as_conflict_without_retrying() {
    let user_id = user_id();
    let repository = Arc::new(StubUserPreferencesRepository::with_preferences(
        UserPreferences::builder(user_id.clone())
            .interest_theme_ids(vec![uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa6")])
            .safety_toggle_ids(vec![uuid_id("3fa85f64-5717-4562-b3fc-2c963f66afa8")])
            .unit_system(UnitSystem::Metric)
            .revision(7)
            .build(),
    ));
    repository.set_save_failure(StubFailure::RevisionMismatch {
        expected: 7,
        actual: 8,
    });
    let command = DieselUserInterestsCommand::new(repository.clone());

    let err = command
        .set_interests(request(
            user_id,
            vec![interest_theme_id("3fa85f64-5717-4562-b3fc-2c963f66afa7")],
            Some(7),
        ))
        .await
        .expect_err("stale write should surface as conflict");

    assert_eq!(repository.save_call_count(), 1);
    assert_eq!(err.code(), crate::domain::ErrorCode::Conflict);
    assert_eq!(err.message(), "revision mismatch");
    assert_eq!(
        err.details()
            .and_then(|details| details.get("expectedRevision"))
            .and_then(serde_json::Value::as_u64),
        Some(7)
    );
    assert_eq!(
        err.details()
            .and_then(|details| details.get("actualRevision"))
            .and_then(serde_json::Value::as_u64),
        Some(8)
    );
}
