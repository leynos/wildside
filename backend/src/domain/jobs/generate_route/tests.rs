//! Tests for route-generation job payloads.

use chrono::{DateTime, Utc};
use insta::assert_json_snapshot;
use pretty_assertions::assert_eq;
use proptest::prelude::*;
use rstest::{fixture, rstest};
use serde_json::{Value, json};
use uuid::Uuid;

use super::{GenerateRouteJob, GenerateRouteJobV1};
use crate::domain::ports::{
    ROUTE_PREFERENCE_MAX_ITEMS, ROUTE_PREFERENCE_MAX_VALUE_BYTES, RouteCoordinates, RouteLocation,
    RoutePreferences, RouteSubmissionPayload, RouteSubmissionRequest,
};
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
        payload: RouteSubmissionPayload {
            origin: coordinates(51.5074, -0.1278),
            destination: coordinates(51.5014, -0.1419),
            preferences: Some(walking_preferences()),
        },
    }
}

fn coordinates(lat: f64, lng: f64) -> RouteLocation {
    RouteLocation::Coordinates(validated_coordinates(lat, lng))
}

fn validated_coordinates(lat: f64, lng: f64) -> RouteCoordinates {
    match RouteCoordinates::new(lat, lng) {
        Ok(coordinates) => coordinates,
        Err(error) => panic!("generated route coordinates should be valid: {error}"),
    }
}

fn walking_preferences() -> RoutePreferences {
    RoutePreferences {
        mode: Some("walking".to_owned()),
        ..RoutePreferences::default()
    }
}

fn valid_job_payload(request_id: Uuid, enqueued_at: DateTime<Utc>) -> GenerateRouteJobV1 {
    GenerateRouteJobV1 {
        request_id,
        idempotency_key: Some(fixture_idempotency_key()),
        user_id: fixture_user_id(),
        origin: coordinates(51.5074, -0.1278),
        destination: coordinates(51.5014, -0.1419),
        preferences: Some(walking_preferences()),
        enqueued_at,
    }
}

#[rstest]
fn constructor_accepts_coordinate_locations(request_id: Uuid, enqueued_at: DateTime<Utc>) {
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
        payload: RouteSubmissionPayload {
            origin: RouteLocation::Identifier("saved:home".to_owned()),
            destination: RouteLocation::Identifier("poi:work".to_owned()),
            preferences: Some(walking_preferences()),
        },
        ..valid_submission()
    };

    let job = GenerateRouteJob::try_from_submission(&submission, request_id, enqueued_at)
        .expect("string location identifiers should build a route job");
    let GenerateRouteJob::V1(payload) = job;

    assert_eq!(
        payload.origin,
        RouteLocation::Identifier("saved:home".to_owned())
    );
    assert_eq!(
        payload.destination,
        RouteLocation::Identifier("poi:work".to_owned())
    );
}

#[rstest]
#[case::boolean_origin("origin", json!(true))]
#[case::number_origin("origin", json!(42))]
#[case::array_origin("origin", json!([51.5074, -0.1278]))]
#[case::boolean_destination("destination", json!(false))]
#[case::number_destination("destination", json!(42.0))]
#[case::array_destination("destination", json!([51.5014, -0.1419]))]
fn serde_rejects_invalid_location_shapes(
    #[case] field: &'static str,
    #[case] invalid_location: Value,
    request_id: Uuid,
    enqueued_at: DateTime<Utc>,
) {
    let job = GenerateRouteJob::v1(valid_job_payload(request_id, enqueued_at))
        .expect("fixture job should be valid");
    let mut value = serde_json::to_value(job).expect("job should serialize");
    value[field] = invalid_location;

    let error = serde_json::from_value::<GenerateRouteJob>(value)
        .expect_err("invalid persisted location shapes should be rejected");

    assert!(
        error
            .to_string()
            .contains("a location identifier string or coordinate object"),
        "unexpected location-shape error: {error}"
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
fn serde_rejects_unsupported_envelope_version(request_id: Uuid, enqueued_at: DateTime<Utc>) {
    let job = GenerateRouteJob::v1(valid_job_payload(request_id, enqueued_at))
        .expect("fixture job should be valid");
    let mut value = serde_json::to_value(job).expect("job should serialize");
    value["v"] = json!("v2");

    let error = serde_json::from_value::<GenerateRouteJob>(value)
        .expect_err("unsupported envelope versions should be rejected");
    let message = error.to_string();

    assert_eq!(message, "unsupported route-generation job envelope version");
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
    payload.origin = RouteLocation::Identifier("saved:home".to_owned());
    payload.destination = RouteLocation::Identifier("poi:work".to_owned());
    let job = GenerateRouteJob::v1(payload).expect("string identifiers should be valid");

    let value = serde_json::to_value(&job).expect("job should serialize");
    let decoded: GenerateRouteJob = serde_json::from_value(value).expect("job should deserialize");

    assert_eq!(decoded, job);
}

#[rstest]
fn preferences_serialize_with_camel_case_field_names(request_id: Uuid, enqueued_at: DateTime<Utc>) {
    let mut payload = valid_job_payload(request_id, enqueued_at);
    payload.preferences = Some(RoutePreferences {
        mode: Some("walking".to_owned()),
        themes: Some(vec!["heritage".to_owned()]),
        theme_ids: Some(vec!["theme-1".to_owned()]),
        interest_theme_ids: Some(vec!["interest-1".to_owned()]),
        avoid: Some(vec!["motorway".to_owned()]),
        avoid_stairs: Some(true),
    });
    let job = GenerateRouteJob::v1(payload).expect("all preference fields should be valid");

    let value = serde_json::to_value(job).expect("job should serialize");
    let preferences = &value["preferences"];

    assert_eq!(preferences["themeIds"], json!(["theme-1"]));
    assert_eq!(preferences["interestThemeIds"], json!(["interest-1"]));
    assert_eq!(preferences["avoidStairs"], json!(true));
}

#[rstest]
#[case::null_origin("origin", "origin must not be null")]
#[case::null_destination("destination", "destination must not be null")]
#[case::null_preferences("preferences", "preferences must not be null")]
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
fn serde_rejects_unbounded_preferences(request_id: Uuid, enqueued_at: DateTime<Utc>) {
    let job = GenerateRouteJob::v1(valid_job_payload(request_id, enqueued_at))
        .expect("fixture job should be valid");
    let mut value = serde_json::to_value(job).expect("job should serialize");
    let preferences = value["preferences"]
        .as_object_mut()
        .expect("fixture job should contain preferences");
    preferences.insert(
        "themes".to_owned(),
        json!(vec!["heritage"; ROUTE_PREFERENCE_MAX_ITEMS + 1]),
    );

    let error = serde_json::from_value::<GenerateRouteJob>(value)
        .expect_err("persisted jobs with excess preferences should be rejected");

    assert!(error.to_string().contains("route preference list exceeds"));

    let mut value = serde_json::to_value(
        GenerateRouteJob::v1(valid_job_payload(request_id, enqueued_at))
            .expect("fixture job should be valid"),
    )
    .expect("job should serialize");
    value["preferences"]["mode"] = json!("é".repeat((ROUTE_PREFERENCE_MAX_VALUE_BYTES / 2) + 1));

    let error = serde_json::from_value::<GenerateRouteJob>(value)
        .expect_err("persisted jobs with overlong preferences should be rejected");

    assert!(error.to_string().contains("route preference value exceeds"));
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

fn location_strategy() -> impl Strategy<Value = RouteLocation> {
    prop_oneof![
        (-90.0_f64..=90.0, -180.0_f64..=180.0)
            .prop_map(|(lat, lng)| RouteLocation::Coordinates(validated_coordinates(lat, lng))),
        "[a-zA-Z0-9:_-]{1,32}".prop_map(RouteLocation::Identifier),
    ]
}

fn preferences_strategy() -> impl Strategy<Value = RoutePreferences> {
    (
        prop::option::of("[a-zA-Z0-9_-]{1,16}"),
        optional_string_list_strategy(),
        optional_string_list_strategy(),
        optional_string_list_strategy(),
        optional_string_list_strategy(),
        prop::option::of(any::<bool>()),
    )
        .prop_map(
            |(mode, themes, theme_ids, interest_theme_ids, avoid, avoid_stairs)| RoutePreferences {
                mode,
                themes,
                theme_ids,
                interest_theme_ids,
                avoid,
                avoid_stairs,
            },
        )
}

fn optional_string_list_strategy() -> impl Strategy<Value = Option<Vec<String>>> {
    prop::option::of(prop::collection::vec("[a-zA-Z0-9_-]{1,16}", 0..4))
}

fn generate_route_job_strategy() -> impl Strategy<Value = GenerateRouteJob> {
    (
        location_strategy(),
        location_strategy(),
        prop::option::of(preferences_strategy()),
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
