//! Tests for route-generation job payloads.

use chrono::{DateTime, Utc};
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

fn valid_job_payload(request_id: Uuid, enqueued_at: DateTime<Utc>) -> GenerateRouteJobV1 {
    GenerateRouteJobV1 {
        request_id,
        idempotency_key: Some(fixture_idempotency_key()),
        user_id: fixture_user_id(),
        origin: json!({ "lat": 51.5074, "lng": -0.1278 }),
        destination: json!({ "lat": 51.5014, "lng": -0.1419 }),
        preferences: Some(json!({ "mode": "walking" })),
        enqueued_at,
    }
}

#[rstest]
fn constructor_accepts_well_formed_submission(request_id: Uuid, enqueued_at: DateTime<Utc>) {
    let job = GenerateRouteJob::try_from_submission(&valid_submission(), request_id, enqueued_at)
        .expect("valid submission should build a route-generation job");

    assert_eq!(
        job,
        GenerateRouteJob::V1(valid_job_payload(request_id, enqueued_at))
    );
}

#[rstest]
fn constructor_accepts_string_location_identifiers(request_id: Uuid, enqueued_at: DateTime<Utc>) {
    let submission = RouteSubmissionRequest {
        payload: json!({
            "origin": "saved:home",
            "destination": "poi:work",
            "preferences": { "mode": "walking" }
        }),
        ..valid_submission()
    };

    let job = GenerateRouteJob::try_from_submission(&submission, request_id, enqueued_at)
        .expect("string location identifiers should build a route job");
    let GenerateRouteJob::V1(payload) = job;

    assert_eq!(payload.origin, json!("saved:home"));
    assert_eq!(payload.destination, json!("poi:work"));
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
#[case::missing_origin("origin", false)]
#[case::missing_destination("destination", false)]
#[case::null_origin("origin", true)]
#[case::null_destination("destination", true)]
fn constructor_rejects_missing_required_fields(
    #[case] missing_field: &'static str,
    #[case] should_set_null: bool,
    request_id: Uuid,
    enqueued_at: DateTime<Utc>,
) {
    let mut payload = valid_submission().payload;
    let payload_object = payload
        .as_object_mut()
        .expect("fixture payload must be an object");
    if should_set_null {
        payload_object.insert(missing_field.to_owned(), Value::Null);
    } else {
        payload_object.remove(missing_field);
    }
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
fn constructor_rejects_null_preferences(request_id: Uuid, enqueued_at: DateTime<Utc>) {
    let mut submission = valid_submission();
    submission.payload["preferences"] = Value::Null;

    let error = GenerateRouteJob::try_from_submission(&submission, request_id, enqueued_at)
        .expect_err("null preferences should be rejected");

    assert_eq!(
        error,
        GenerateRouteJobBuildError::PayloadNullField {
            field: "preferences"
        }
    );
}

#[rstest]
#[case::null_origin(
    "origin",
    GenerateRouteJobBuildError::PayloadMissingField { field: "origin" }
)]
#[case::null_destination(
    "destination",
    GenerateRouteJobBuildError::PayloadMissingField { field: "destination" }
)]
#[case::null_preferences(
    "preferences",
    GenerateRouteJobBuildError::PayloadNullField { field: "preferences" }
)]
fn v1_rejects_null_payload_fields(
    #[case] field: &str,
    #[case] expected: GenerateRouteJobBuildError,
    request_id: Uuid,
    enqueued_at: DateTime<Utc>,
) {
    let mut payload = valid_job_payload(request_id, enqueued_at);
    match field {
        "origin" => payload.origin = Value::Null,
        "destination" => payload.destination = Value::Null,
        "preferences" => payload.preferences = Some(Value::Null),
        _ => panic!("unsupported test field: {field}"),
    }

    let error = GenerateRouteJob::v1(payload).expect_err("null payload fields should be rejected");

    assert_eq!(error, expected);
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
fn absent_preferences_round_trip_without_null(request_id: Uuid, enqueued_at: DateTime<Utc>) {
    let mut payload = valid_job_payload(request_id, enqueued_at);
    payload.preferences = None;
    let job = GenerateRouteJob::v1(payload).expect("absent preferences should be valid");

    let value = serde_json::to_value(&job).expect("job should serialize");
    assert!(
        value.get("preferences").is_none(),
        "absent preferences should be omitted from the wire payload"
    );
    let decoded: GenerateRouteJob = serde_json::from_value(value).expect("job should deserialize");

    assert_eq!(decoded, job);
}

#[rstest]
fn string_location_identifiers_round_trip(request_id: Uuid, enqueued_at: DateTime<Utc>) {
    let mut payload = valid_job_payload(request_id, enqueued_at);
    payload.origin = json!("saved:home");
    payload.destination = json!("poi:work");
    let job = GenerateRouteJob::v1(payload).expect("string identifiers should be valid");

    let value = serde_json::to_value(&job).expect("job should serialize");
    let decoded: GenerateRouteJob = serde_json::from_value(value).expect("job should deserialize");

    assert_eq!(decoded, job);
}

#[rstest]
#[case::null_origin("origin", "route job payload is missing required field: origin")]
#[case::null_destination(
    "destination",
    "route job payload is missing required field: destination"
)]
#[case::null_preferences("preferences", "route job payload field must not be null: preferences")]
fn serde_rejects_null_payload_fields(
    #[case] field: &str,
    #[case] expected_message: &str,
    request_id: Uuid,
    enqueued_at: DateTime<Utc>,
) {
    let job = GenerateRouteJob::v1(valid_job_payload(request_id, enqueued_at))
        .expect("fixture job should be valid");
    let mut value = serde_json::to_value(job).expect("job should serialize");
    value[field] = Value::Null;

    let error = serde_json::from_value::<GenerateRouteJob>(value)
        .expect_err("persisted null payload fields should be rejected");

    assert!(
        error.to_string().contains(expected_message),
        "expected error to contain {expected_message:?}, got {error}"
    );
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

fn location_strategy() -> impl Strategy<Value = Value> {
    prop_oneof![
        json_object_strategy(),
        "[a-zA-Z0-9:_-]{1,32}".prop_map(Value::String),
    ]
}

fn generate_route_job_strategy() -> impl Strategy<Value = GenerateRouteJob> {
    (
        location_strategy(),
        location_strategy(),
        prop::option::of(json_object_strategy()),
    )
        .prop_map(|(origin, destination, preferences)| {
            let job = GenerateRouteJob::v1(GenerateRouteJobV1 {
                request_id: Uuid::nil(),
                idempotency_key: Some(fixture_idempotency_key()),
                user_id: fixture_user_id(),
                origin,
                destination,
                preferences,
                enqueued_at: fixture_enqueued_at(),
            });
            match job {
                Ok(job) => job,
                Err(error) => {
                    panic!("strategy should generate valid route job payloads: {error}")
                }
            }
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
