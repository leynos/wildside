//! Unit tests for the route submission service.
//!
//! Tests cover idempotency key handling, payload hash matching, conflict
//! detection, and concurrent insert race conditions.

use std::sync::Arc;

use chrono::Utc;
use mockall::predicate::*;
use serde_json::json;
use uuid::Uuid;

use super::RouteSubmissionServiceImpl;
use crate::domain::ports::{
    FixtureIdempotencyStore, IdempotencyStoreError, MockIdempotencyStore, RouteSubmissionRequest,
    RouteSubmissionResponse, RouteSubmissionService, RouteSubmissionStatus,
};
use crate::domain::{
    IdempotencyKey, IdempotencyLookupResult, IdempotencyRecord, PayloadHash, UserId,
    canonicalize_and_hash,
};

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
) -> IdempotencyRecord {
    let response = RouteSubmissionResponse::accepted(request_id);
    let response_snapshot = serde_json::to_value(&response).expect("serialization should succeed");

    IdempotencyRecord {
        key,
        payload_hash,
        response_snapshot,
        user_id,
        created_at: Utc::now(),
    }
}

/// Default test payload.
fn default_payload() -> serde_json::Value {
    json!({"origin": "A", "destination": "B"})
}

/// Alternative test payload (different from default).
fn alternative_payload() -> serde_json::Value {
    json!({"origin": "X", "destination": "Y"})
}

/// Helper to configure a mock store that returns a specific lookup result.
fn expect_lookup_returns(
    mock: &mut MockIdempotencyStore,
    key: IdempotencyKey,
    user_id: UserId,
    result: IdempotencyLookupResult,
) {
    mock.expect_lookup()
        .with(eq(key), eq(user_id), always())
        .times(1)
        .return_once(move |_, _, _| Ok(result));
}

/// Helper to configure a mock store that returns NotFound.
fn expect_lookup_not_found(mock: &mut MockIdempotencyStore, key: IdempotencyKey, user_id: UserId) {
    expect_lookup_returns(mock, key, user_id, IdempotencyLookupResult::NotFound);
}

/// Helper to configure a mock store that fails with DuplicateKey on store.
fn expect_store_duplicate_key(mock: &mut MockIdempotencyStore) {
    mock.expect_store()
        .times(1)
        .return_once(|_| Err(IdempotencyStoreError::duplicate_key("concurrent insert")));
}

/// Helper to set up a concurrent insert race condition test scenario.
/// Returns the configured service and request for the test to execute and assert.
fn setup_race_condition_test(
    is_matching_payload: bool,
    our_payload: serde_json::Value,
    their_hash: PayloadHash,
    their_request_id: Uuid,
) -> (
    RouteSubmissionServiceImpl<MockIdempotencyStore>,
    RouteSubmissionRequest,
) {
    let idempotency_key = IdempotencyKey::random();
    let user_id = UserId::random();

    let their_record = build_record(
        idempotency_key.clone(),
        their_hash,
        their_request_id,
        user_id.clone(),
    );

    let mut mock_store = MockIdempotencyStore::new();

    // First lookup returns NotFound (simulating a race where another request
    // inserted between our lookup and store).
    expect_lookup_not_found(&mut mock_store, idempotency_key.clone(), user_id.clone());

    // Store fails with DuplicateKey (the other request won the race).
    expect_store_duplicate_key(&mut mock_store);

    // Retry lookup after race returns either MatchingPayload or ConflictingPayload.
    let retry_result = if is_matching_payload {
        IdempotencyLookupResult::MatchingPayload(their_record)
    } else {
        IdempotencyLookupResult::ConflictingPayload(their_record)
    };

    expect_lookup_returns(
        &mut mock_store,
        idempotency_key.clone(),
        user_id.clone(),
        retry_result,
    );

    let service = RouteSubmissionServiceImpl::with_noop_metrics(Arc::new(mock_store));
    let request = build_request(Some(idempotency_key), user_id, our_payload);

    (service, request)
}

fn make_service() -> RouteSubmissionServiceImpl<FixtureIdempotencyStore> {
    RouteSubmissionServiceImpl::with_noop_metrics(Arc::new(FixtureIdempotencyStore))
}

#[tokio::test]
async fn accepts_request_without_idempotency_key() {
    let service = make_service();
    let request = build_request(None, UserId::random(), default_payload());

    let response = service
        .submit(request)
        .await
        .expect("submission should succeed");
    assert_eq!(response.status, RouteSubmissionStatus::Accepted);
}

#[tokio::test]
async fn accepts_request_with_new_idempotency_key() {
    let service = make_service();
    let request = build_request(
        Some(IdempotencyKey::random()),
        UserId::random(),
        default_payload(),
    );

    // FixtureIdempotencyStore always returns NotFound, so new keys are accepted.
    let response = service
        .submit(request)
        .await
        .expect("submission should succeed");
    assert_eq!(response.status, RouteSubmissionStatus::Accepted);
}

#[tokio::test]
async fn replays_response_for_matching_payload() {
    let idempotency_key = IdempotencyKey::random();
    let user_id = UserId::random();
    let payload = default_payload();
    let payload_hash = canonicalize_and_hash(&payload);
    let original_request_id = Uuid::new_v4();

    let existing_record = build_record(
        idempotency_key.clone(),
        payload_hash,
        original_request_id,
        user_id.clone(),
    );

    let mut mock_store = MockIdempotencyStore::new();
    expect_lookup_returns(
        &mut mock_store,
        idempotency_key.clone(),
        user_id.clone(),
        IdempotencyLookupResult::MatchingPayload(existing_record),
    );

    let service = RouteSubmissionServiceImpl::with_noop_metrics(Arc::new(mock_store));
    let request = build_request(Some(idempotency_key), user_id, payload);

    let response = service
        .submit(request)
        .await
        .expect("submission should succeed");

    assert_eq!(response.status, RouteSubmissionStatus::Replayed);
    assert_eq!(response.request_id, original_request_id);
}

#[tokio::test]
async fn returns_conflict_for_different_payload() {
    let idempotency_key = IdempotencyKey::random();
    let user_id = UserId::random();
    let original_hash = canonicalize_and_hash(&default_payload());

    let existing_record = build_record(
        idempotency_key.clone(),
        original_hash,
        Uuid::new_v4(),
        user_id.clone(),
    );

    let mut mock_store = MockIdempotencyStore::new();
    expect_lookup_returns(
        &mut mock_store,
        idempotency_key.clone(),
        user_id.clone(),
        IdempotencyLookupResult::ConflictingPayload(existing_record),
    );

    let service = RouteSubmissionServiceImpl::with_noop_metrics(Arc::new(mock_store));
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
}

#[tokio::test]
async fn handles_concurrent_insert_race_with_matching_payload() {
    let payload = default_payload();
    let payload_hash = canonicalize_and_hash(&payload);
    let original_request_id = Uuid::new_v4();

    let (service, request) = setup_race_condition_test(
        true, // is_matching_payload
        payload,
        payload_hash,
        original_request_id,
    );

    let response = service
        .submit(request)
        .await
        .expect("submission should succeed after race resolution");

    assert_eq!(response.status, RouteSubmissionStatus::Replayed);
    assert_eq!(response.request_id, original_request_id);
}

#[tokio::test]
async fn handles_concurrent_insert_race_with_conflicting_payload() {
    let our_payload = default_payload();
    let their_hash = canonicalize_and_hash(&alternative_payload());

    let (service, request) = setup_race_condition_test(
        false, // is_matching_payload
        our_payload,
        their_hash,
        Uuid::new_v4(), // their_request_id doesn't matter for conflict case
    );

    let error = service
        .submit(request)
        .await
        .expect_err("submission should fail with conflict");

    assert_eq!(error.code(), crate::domain::ErrorCode::Conflict);
}

// Tests for helper functions

mod age_bucket_tests {
    use chrono::{Duration, TimeZone, Utc};
    use rstest::rstest;

    use super::super::calculate_age_bucket;

    /// Fixed reference time for deterministic tests.
    fn fixed_now() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2025, 1, 15, 12, 0, 0).unwrap()
    }

    /// Parameterized test for age bucket boundary values.
    ///
    /// Tests cover all bucket boundaries and edge cases:
    /// - `0-1m`: 0 to 59 seconds
    /// - `1-5m`: 1 to 4 minutes
    /// - `5-30m`: 5 to 29 minutes
    /// - `30m-2h`: 30 to 119 minutes
    /// - `2h-6h`: 120 to 359 minutes
    /// - `6h-24h`: 360 to 1439 minutes
    /// - `>24h`: 1440+ minutes
    /// - Future timestamps (clock skew) clamp to `0-1m`
    #[rstest]
    #[case::zero_seconds(0, "0-1m")]
    #[case::thirty_seconds(30, "0-1m")]
    #[case::one_minute(60, "1-5m")]
    #[case::four_minutes(4 * 60, "1-5m")]
    #[case::five_minutes(5 * 60, "5-30m")]
    #[case::twenty_nine_minutes(29 * 60, "5-30m")]
    #[case::thirty_minutes(30 * 60, "30m-2h")]
    #[case::one_hour(60 * 60, "30m-2h")]
    #[case::two_hours(2 * 60 * 60, "2h-6h")]
    #[case::five_hours(5 * 60 * 60, "2h-6h")]
    #[case::six_hours(6 * 60 * 60, "6h-24h")]
    #[case::twenty_three_hours(23 * 60 * 60, "6h-24h")]
    #[case::twenty_four_hours(24 * 60 * 60, ">24h")]
    #[case::forty_eight_hours(48 * 60 * 60, ">24h")]
    #[case::future_timestamp_clamps(-5 * 60, "0-1m")]
    fn age_bucket_boundaries(#[case] offset_seconds: i64, #[case] expected: &str) {
        let now = fixed_now();
        let created = now - Duration::seconds(offset_seconds);
        assert_eq!(calculate_age_bucket(created, now), expected);
    }
}

mod user_scope_hash_tests {
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
