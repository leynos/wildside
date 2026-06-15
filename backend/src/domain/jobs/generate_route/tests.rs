//! Tests for route-generation job payloads.

use chrono::{DateTime, Utc};
use googletest::prelude::*;
use insta::assert_json_snapshot;
use pretty_assertions::assert_eq;
use proptest::prelude::*;
use rstest::{fixture, rstest};
use serde_json::{Value, json};
use uuid::Uuid;

use super::{GenerateRouteJob, GenerateRouteJobBuildError, GenerateRouteJobV1};
use crate::domain::ports::RouteSubmissionRequest;
use crate::domain::{IdempotencyKey, UserId};

#[fixture]
fn request_id() -> Uuid {
    Uuid::from_bytes([0x11; 16])
}

#[fixture]
fn enqueued_at() -> DateTime<Utc> {
    fixture_enqueued_at()
}

fn fixture_user_id() -> UserId {
    UserId::from_uuid(Uuid::from_bytes([0x22; 16]))
}

fn fixture_idempotency_key() -> IdempotencyKey {
    IdempotencyKey::from_uuid(Uuid::from_bytes([0x33; 16]))
}

fn fixture_enqueued_at() -> DateTime<Utc> {
    match DateTime::parse_from_rfc3339("2026-06-14T12:00:00Z") {
        Ok(timestamp) => timestamp.with_timezone(&Utc),
        Err(error) => panic!("static enqueue timestamp must be valid: {error}"),
    }
}

fn valid_submission() -> RouteSubmissionRequest {
    RouteSubmissionRequest {
        idempotency_key: Some(fixture_idempotency_key()),
        user_id: fixture_user_id(),
        payload: json!({
            "origin": { "lat": 51.5074, "lng": -0.1278 },
            "destination": { "lat": 51.5014, "lng": -0.1419 },
            "preferences": { "mode": "walking" }
        }),
    }
}

#[rstest]
fn constructor_accepts_well_formed_submission(request_id: Uuid, enqueued_at: DateTime<Utc>) {
    let job = GenerateRouteJob::try_from_submission(&valid_submission(), request_id, enqueued_at)
        .expect("valid submission should build a route-generation job");

    assert_that!(
        job,
        eq(&GenerateRouteJob::V1(GenerateRouteJobV1 {
            request_id,
            idempotency_key: Some(fixture_idempotency_key()),
            user_id: fixture_user_id(),
            origin: json!({ "lat": 51.5074, "lng": -0.1278 }),
            destination: json!({ "lat": 51.5014, "lng": -0.1419 }),
            preferences: Some(json!({ "mode": "walking" })),
            enqueued_at,
        }))
    );
}

#[rstest]
fn constructor_rejects_payloads_that_are_not_objects(request_id: Uuid, enqueued_at: DateTime<Utc>) {
    let submission = RouteSubmissionRequest {
        payload: json!("not an object"),
        ..valid_submission()
    };

    let error = GenerateRouteJob::try_from_submission(&submission, request_id, enqueued_at)
        .expect_err("non-object payload should be rejected");

    assert_eq!(error, GenerateRouteJobBuildError::PayloadNotObject);
}

#[rstest]
#[case("origin")]
#[case("destination")]
fn constructor_rejects_missing_required_fields(
    #[case] missing_field: &'static str,
    request_id: Uuid,
    enqueued_at: DateTime<Utc>,
) {
    let mut payload = valid_submission().payload;
    payload
        .as_object_mut()
        .expect("fixture payload must be an object")
        .remove(missing_field);
    let submission = RouteSubmissionRequest {
        payload,
        ..valid_submission()
    };

    let error = GenerateRouteJob::try_from_submission(&submission, request_id, enqueued_at)
        .expect_err("missing required field should be rejected");

    assert_eq!(
        error,
        GenerateRouteJobBuildError::PayloadMissingField {
            field: missing_field
        }
    );
}

#[rstest]
fn serde_round_trip_is_identity(request_id: Uuid, enqueued_at: DateTime<Utc>) {
    let job = GenerateRouteJob::try_from_submission(&valid_submission(), request_id, enqueued_at)
        .expect("valid submission should build a route-generation job");

    let value = serde_json::to_value(&job).expect("job should serialize");
    let decoded: GenerateRouteJob = serde_json::from_value(value).expect("job should deserialize");

    assert_eq!(decoded, job);
}

#[rstest]
fn unknown_fields_are_rejected(request_id: Uuid, enqueued_at: DateTime<Utc>) {
    let job = GenerateRouteJob::try_from_submission(&valid_submission(), request_id, enqueued_at)
        .expect("valid submission should build a route-generation job");
    let mut value = serde_json::to_value(job).expect("job should serialize");
    value
        .as_object_mut()
        .expect("job envelope should be an object")
        .insert("unexpected".to_owned(), json!(true));

    let error = serde_json::from_value::<GenerateRouteJob>(value)
        .expect_err("unknown V1 fields should be rejected");

    assert!(error.to_string().contains("unknown field"));
}

#[rstest]
fn snapshot_locks_v1_json_shape(request_id: Uuid, enqueued_at: DateTime<Utc>) {
    let job = GenerateRouteJob::try_from_submission(&valid_submission(), request_id, enqueued_at)
        .expect("valid submission should build a route-generation job");
    let value = serde_json::to_value(job).expect("job should serialize");

    assert_json_snapshot!("generate_route_job_v1", value);
}

fn json_leaf_strategy() -> impl Strategy<Value = Value> {
    prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        (-10_000_i64..=10_000_i64).prop_map(|number| json!(number)),
        "[a-zA-Z0-9 _.-]{0,32}".prop_map(Value::String),
    ]
}

fn json_object_strategy() -> impl Strategy<Value = Value> {
    prop::collection::btree_map("[a-z][a-z0-9_]{0,15}", json_leaf_strategy(), 0..8)
        .prop_map(|object| object.into_iter().collect())
        .prop_map(Value::Object)
}

fn generate_route_job_strategy() -> impl Strategy<Value = GenerateRouteJob> {
    (
        json_object_strategy(),
        json_object_strategy(),
        prop::option::of(json_object_strategy()),
    )
        .prop_map(|(origin, destination, preferences)| {
            GenerateRouteJob::v1(
                Uuid::nil(),
                Some(fixture_idempotency_key()),
                fixture_user_id(),
                origin,
                destination,
                preferences,
                fixture_enqueued_at(),
            )
        })
}

proptest! {
    #[test]
    fn generated_jobs_round_trip_through_json(job in generate_route_job_strategy()) {
        let value = serde_json::to_value(&job).expect("generated job should serialize");
        let decoded: GenerateRouteJob =
            serde_json::from_value(value).expect("generated job should deserialize");

        prop_assert_eq!(decoded, job);
    }
}
