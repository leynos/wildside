//! Tests for the user preferences service.

use std::sync::Arc;

use super::*;
use crate::domain::ports::{
    FixtureIdempotencyRepository, MockIdempotencyRepository, MockUserPreferencesRepository,
};
use crate::domain::{
    IdempotencyKey, IdempotencyLookupQuery, IdempotencyLookupResult, IdempotencyRecord,
    MutationType, UnitSystem, UserId, UserPreferences,
};
use chrono::Utc;
use uuid::Uuid;

fn make_service(
    repo: MockUserPreferencesRepository,
) -> UserPreferencesService<MockUserPreferencesRepository, FixtureIdempotencyRepository> {
    UserPreferencesService::new(Arc::new(repo), Arc::new(FixtureIdempotencyRepository))
}

#[tokio::test]
async fn update_creates_preferences_when_missing() {
    let user_id = UserId::random();
    let mut repo = MockUserPreferencesRepository::new();

    repo.expect_find_by_user_id()
        .times(1)
        .return_once(|_| Ok(None));
    repo.expect_save().times(1).return_once(|_, _| Ok(()));

    let service = make_service(repo);
    let request = UpdatePreferencesRequest {
        user_id: user_id.clone(),
        interest_theme_ids: Vec::new(),
        safety_toggle_ids: Vec::new(),
        unit_system: UnitSystem::Metric,
        expected_revision: None,
        idempotency_key: None,
    };

    let response = service.update(request).await.expect("update succeeds");
    assert_eq!(response.preferences.user_id, user_id);
    assert_eq!(response.preferences.revision, 1);
    assert!(!response.replayed);
}

#[tokio::test]
async fn update_rejects_missing_revision_when_record_exists() {
    let user_id = UserId::random();
    let existing = UserPreferences::builder(user_id.clone())
        .revision(3)
        .build();
    let mut repo = MockUserPreferencesRepository::new();

    repo.expect_find_by_user_id()
        .times(1)
        .return_once(move |_| Ok(Some(existing)));

    let service = make_service(repo);
    let request = UpdatePreferencesRequest {
        user_id,
        interest_theme_ids: Vec::new(),
        safety_toggle_ids: Vec::new(),
        unit_system: UnitSystem::Metric,
        expected_revision: None,
        idempotency_key: None,
    };

    let error = service.update(request).await.expect_err("conflict");
    assert_eq!(error.code(), crate::domain::ErrorCode::Conflict);
}

#[tokio::test]
async fn update_rejects_revision_mismatch() {
    let user_id = UserId::random();
    let existing = UserPreferences::builder(user_id.clone())
        .revision(2)
        .build();
    let mut repo = MockUserPreferencesRepository::new();

    repo.expect_find_by_user_id()
        .times(1)
        .return_once(move |_| Ok(Some(existing)));

    let service = make_service(repo);
    let request = UpdatePreferencesRequest {
        user_id,
        interest_theme_ids: Vec::new(),
        safety_toggle_ids: Vec::new(),
        unit_system: UnitSystem::Metric,
        expected_revision: Some(1),
        idempotency_key: None,
    };

    let error = service.update(request).await.expect_err("conflict");
    assert_eq!(error.code(), crate::domain::ErrorCode::Conflict);
}

#[tokio::test]
async fn update_returns_cached_response_for_duplicate_idempotency_key() {
    let user_id = UserId::random();
    let idempotency_key = IdempotencyKey::random();
    let request = UpdatePreferencesRequest {
        user_id: user_id.clone(),
        interest_theme_ids: vec![Uuid::new_v4()],
        safety_toggle_ids: vec![Uuid::new_v4()],
        unit_system: UnitSystem::Metric,
        expected_revision: Some(1),
        idempotency_key: Some(idempotency_key.clone()),
    };
    let payload_hash = UserPreferencesService::<
        MockUserPreferencesRepository,
        MockIdempotencyRepository,
    >::preferences_payload_hash(&request);
    let preferences = UserPreferences::builder(user_id.clone())
        .revision(2)
        .unit_system(UnitSystem::Metric)
        .build();
    let response_snapshot = serde_json::to_value(UpdatePreferencesResponse {
        preferences: preferences.clone(),
        replayed: false,
    })
    .expect("response snapshot");
    let record = IdempotencyRecord {
        key: idempotency_key.clone(),
        mutation_type: MutationType::Preferences,
        payload_hash: payload_hash.clone(),
        response_snapshot,
        user_id: user_id.clone(),
        created_at: Utc::now(),
    };

    let mut preferences_repo = MockUserPreferencesRepository::new();
    preferences_repo.expect_find_by_user_id().times(0);
    preferences_repo.expect_save().times(0);

    let mut idempotency_repo = MockIdempotencyRepository::new();
    idempotency_repo
        .expect_lookup()
        .withf(move |query: &IdempotencyLookupQuery| {
            query.key == idempotency_key
                && query.user_id == user_id
                && query.mutation_type == MutationType::Preferences
                && query.payload_hash == payload_hash
        })
        .times(1)
        .return_once(move |_| Ok(IdempotencyLookupResult::MatchingPayload(record)));
    idempotency_repo.expect_store().times(0);

    let service =
        UserPreferencesService::new(Arc::new(preferences_repo), Arc::new(idempotency_repo));

    let response = service.update(request).await.expect("cached response");
    assert_eq!(response.preferences.revision, 2);
    assert!(response.replayed);
}
