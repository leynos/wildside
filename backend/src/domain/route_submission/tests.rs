//! Unit tests for the route submission service.
//!
//! Tests cover idempotency key handling, payload hash matching, conflict
//! detection, and concurrent insert race conditions.

#![cfg(test)]

use std::{error::Error as StdError, sync::Arc};

use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use super::RouteSubmissionServiceImpl;
use crate::domain::ports::{
    FixtureIdempotencyRepository, IdempotencyRepositoryError, MockIdempotencyRepository,
    RouteSubmissionRequest, RouteSubmissionResponse, RouteSubmissionService, RouteSubmissionStatus,
};
use crate::domain::{
    IdempotencyKey, IdempotencyLookupQuery, IdempotencyLookupResult, IdempotencyRecord,
    MutationType, PayloadHash, UserId, canonicalize_and_hash,
};

type TestResult<T = ()> = Result<T, Box<dyn StdError>>;

mod age_bucket_tests;

/// Helper to build a RouteSubmissionRequest for tests.
fn build_request(
    idempotency_key: Option<IdempotencyKey>,
    user_id: UserId,
    payload: serde_json::Value,
) -> RouteSubmissionRequest {
    RouteSubmissionRequest {
        idempotency_key,
        user_id,
        payload,
    }
}

/// Helper to build an IdempotencyRecord fixture.
fn build_record(
    key: IdempotencyKey,
    payload_hash: PayloadHash,
    request_id: Uuid,
    user_id: UserId,
) -> TestResult<IdempotencyRecord> {
    let response = RouteSubmissionResponse::accepted(request_id);
    let response_snapshot = serde_json::to_value(&response)?;

    Ok(IdempotencyRecord {
        key,
        mutation_type: MutationType::Routes,
        payload_hash,
        response_snapshot,
        user_id,
        created_at: Utc::now(),
    })
}

/// Default test payload.
fn default_payload() -> serde_json::Value {
    json!({"origin": "A", "destination": "B"})
}

/// Alternative test payload (different from default).
fn alternative_payload() -> serde_json::Value {
    json!({"origin": "X", "destination": "Y"})
}

/// Helper to configure a mock repository that returns a specific lookup result.
fn expect_lookup_returns(
    mock: &mut MockIdempotencyRepository,
    key: IdempotencyKey,
    user_id: UserId,
    result: IdempotencyLookupResult,
) {
    mock.expect_lookup()
        .withf(move |query: &IdempotencyLookupQuery| {
            query.key == key
                && query.user_id == user_id
                && query.mutation_type == MutationType::Routes
        })
        .times(1)
        .return_once(move |_| Ok(result));
}

/// Helper to configure a mock repository that returns NotFound.
fn expect_lookup_not_found(
    mock: &mut MockIdempotencyRepository,
    key: IdempotencyKey,
    user_id: UserId,
) {
    expect_lookup_returns(mock, key, user_id, IdempotencyLookupResult::NotFound);
}

/// Helper to configure a mock repository that fails with DuplicateKey on store.
fn expect_store_duplicate_key(mock: &mut MockIdempotencyRepository) {
    mock.expect_store().times(1).return_once(|_| {
        Err(IdempotencyRepositoryError::duplicate_key(
            "concurrent insert",
        ))
    });
}

/// Helper to set up a concurrent insert race condition test scenario.
/// Returns the configured service and request for the test to execute and assert.
fn setup_race_condition_test(
    is_matching_payload: bool,
    our_payload: serde_json::Value,
    their_hash: PayloadHash,
    their_request_id: Uuid,
) -> TestResult<(
    RouteSubmissionServiceImpl<MockIdempotencyRepository>,
    RouteSubmissionRequest,
)> {
    let idempotency_key = IdempotencyKey::random();
    let user_id = UserId::random();

    let their_record = build_record(
        idempotency_key.clone(),
        their_hash,
        their_request_id,
        user_id.clone(),
    )?;

    let mut mock_repo = MockIdempotencyRepository::new();

    // First lookup returns NotFound (simulating a race where another request
    // inserted between our lookup and store).
    expect_lookup_not_found(&mut mock_repo, idempotency_key.clone(), user_id.clone());

    // Store fails with DuplicateKey (the other request won the race).
    expect_store_duplicate_key(&mut mock_repo);

    // Retry lookup after race returns either MatchingPayload or ConflictingPayload.
    let retry_result = if is_matching_payload {
        IdempotencyLookupResult::MatchingPayload(their_record)
    } else {
        IdempotencyLookupResult::ConflictingPayload(their_record)
    };

    expect_lookup_returns(
        &mut mock_repo,
        idempotency_key.clone(),
        user_id.clone(),
        retry_result,
    );

    let service = RouteSubmissionServiceImpl::with_noop_metrics(Arc::new(mock_repo));
    let request = build_request(Some(idempotency_key), user_id, our_payload);

    Ok((service, request))
}

fn make_service() -> RouteSubmissionServiceImpl<FixtureIdempotencyRepository> {
    RouteSubmissionServiceImpl::with_noop_metrics(Arc::new(FixtureIdempotencyRepository))
}

#[tokio::test]
async fn accepts_request_without_idempotency_key() -> TestResult {
    let service = make_service();
    let request = build_request(None, UserId::random(), default_payload());

    let response = service.submit(request).await?;
    assert_eq!(response.status, RouteSubmissionStatus::Accepted);
    Ok(())
}

#[tokio::test]
async fn accepts_request_with_new_idempotency_key() -> TestResult {
    let service = make_service();
    let request = build_request(
        Some(IdempotencyKey::random()),
        UserId::random(),
        default_payload(),
    );

    // FixtureIdempotencyStore always returns NotFound, so new keys are accepted.
    let response = service.submit(request).await?;
    assert_eq!(response.status, RouteSubmissionStatus::Accepted);
    Ok(())
}

#[tokio::test]
async fn replays_response_for_matching_payload() -> TestResult {
    let idempotency_key = IdempotencyKey::random();
    let user_id = UserId::random();
    let payload = default_payload();
    let payload_hash = canonicalize_and_hash(&payload)?;
    let original_request_id = Uuid::new_v4();

    let existing_record = build_record(
        idempotency_key.clone(),
        payload_hash,
        original_request_id,
        user_id.clone(),
    )?;

    let mut mock_repo = MockIdempotencyRepository::new();
    expect_lookup_returns(
        &mut mock_repo,
        idempotency_key.clone(),
        user_id.clone(),
        IdempotencyLookupResult::MatchingPayload(existing_record),
    );

    let service = RouteSubmissionServiceImpl::with_noop_metrics(Arc::new(mock_repo));
    let request = build_request(Some(idempotency_key), user_id, payload);

    let response = service.submit(request).await?;

    assert_eq!(response.status, RouteSubmissionStatus::Replayed);
    assert_eq!(response.request_id, original_request_id);
    Ok(())
}

#[tokio::test]
async fn returns_conflict_for_different_payload() -> TestResult {
    let idempotency_key = IdempotencyKey::random();
    let user_id = UserId::random();
    let original_hash = canonicalize_and_hash(&default_payload())?;

    let existing_record = build_record(
        idempotency_key.clone(),
        original_hash,
        Uuid::new_v4(),
        user_id.clone(),
    )?;

    let mut mock_repo = MockIdempotencyRepository::new();
    expect_lookup_returns(
        &mut mock_repo,
        idempotency_key.clone(),
        user_id.clone(),
        IdempotencyLookupResult::ConflictingPayload(existing_record),
    );

    let service = RouteSubmissionServiceImpl::with_noop_metrics(Arc::new(mock_repo));
    let request = build_request(Some(idempotency_key), user_id, alternative_payload());

    let error = service
        .submit(request)
        .await
        .expect_err("submission should fail with conflict");

    assert_eq!(error.code(), crate::domain::ErrorCode::Conflict);
    assert!(
        error
            .message()
            .contains("idempotency key already used with different payload")
    );
    Ok(())
}

#[tokio::test]
async fn handles_concurrent_insert_race_with_matching_payload() -> TestResult {
    let payload = default_payload();
    let payload_hash = canonicalize_and_hash(&payload)?;
    let original_request_id = Uuid::new_v4();

    let (service, request) = setup_race_condition_test(
        true, // is_matching_payload
        payload,
        payload_hash,
        original_request_id,
    )?;

    let response = service.submit(request).await?;

    assert_eq!(response.status, RouteSubmissionStatus::Replayed);
    assert_eq!(response.request_id, original_request_id);
    Ok(())
}

#[tokio::test]
async fn handles_concurrent_insert_race_with_conflicting_payload() -> TestResult {
    let our_payload = default_payload();
    let their_hash = canonicalize_and_hash(&alternative_payload())?;

    let (service, request) = setup_race_condition_test(
        false, // is_matching_payload
        our_payload,
        their_hash,
        Uuid::new_v4(), // their_request_id doesn't matter for conflict case
    )?;

    let error = service
        .submit(request)
        .await
        .expect_err("submission should fail with conflict");

    assert_eq!(error.code(), crate::domain::ErrorCode::Conflict);
    Ok(())
}

mod user_scope_hash_tests {
    //! Coverage for the user scope hash helper.
    use super::super::user_scope_hash;
    use crate::domain::UserId;

    #[test]
    fn returns_8_character_hex_string() {
        let user_id = UserId::random();
        let scope = user_scope_hash(&user_id);

        assert_eq!(scope.len(), 8, "user scope hash should be 8 characters");
        assert!(
            scope.chars().all(|c| c.is_ascii_hexdigit()),
            "user scope hash should contain only hex characters"
        );
    }

    #[test]
    fn returns_lowercase_hex() {
        let user_id = UserId::random();
        let scope = user_scope_hash(&user_id);

        assert_eq!(
            scope,
            scope.to_lowercase(),
            "user scope hash should be lowercase"
        );
    }

    #[test]
    fn is_deterministic_for_same_user() {
        let user_id = UserId::random();
        let scope1 = user_scope_hash(&user_id);
        let scope2 = user_scope_hash(&user_id);

        assert_eq!(scope1, scope2, "same user should produce same hash");
    }

    #[test]
    fn different_users_produce_different_hashes() {
        // Use fixed UUIDs to ensure deterministic test (avoid rare hash collisions
        // with random IDs).
        let user_a = UserId::new("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let user_b = UserId::new("550e8400-e29b-41d4-a716-446655440001").unwrap();

        let scope_a = user_scope_hash(&user_a);
        let scope_b = user_scope_hash(&user_b);

        assert_ne!(
            scope_a, scope_b,
            "different users should produce different hashes"
        );
    }
}
