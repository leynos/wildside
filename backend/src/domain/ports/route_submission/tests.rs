//! Regression coverage for the route-submission driving port.
//!
//! The coordinate cases reach the field rules through the public
//! [`RouteLocation`](super::RouteLocation) deserializer, which is the only way
//! the private `CoordinateFields` collector is exercised in production, so a
//! regression in those rules fails here rather than surfacing as a malformed
//! request reaching route generation.

use rstest::rstest;

use super::*;

/// Parse `json` as a [`RouteLocation`], reporting the deserializer's error.
fn parse_location(json: &str) -> Result<RouteLocation, serde_json::Error> {
    serde_json::from_str(json)
}

fn route_payload() -> RouteSubmissionPayload {
    RouteSubmissionPayload {
        origin: RouteLocation::Identifier("A".to_owned()),
        destination: RouteLocation::Identifier("B".to_owned()),
        preferences: None,
    }
}

#[tokio::test]
async fn fixture_service_accepts_requests() {
    let service = FixtureRouteSubmissionService;
    let request = RouteSubmissionRequest {
        idempotency_key: None,
        user_id: UserId::random(),
        payload: route_payload(),
    };

    let response = service
        .submit(request)
        .await
        .expect("submit should succeed");
    assert_eq!(
        response.status,
        RouteSubmissionStatus::Accepted,
        "the fixture service should accept a request that carries no idempotency key"
    );
}

#[tokio::test]
async fn fixture_service_accepts_requests_with_idempotency_key() {
    let service = FixtureRouteSubmissionService;
    let request = RouteSubmissionRequest {
        idempotency_key: Some(IdempotencyKey::random()),
        user_id: UserId::random(),
        payload: route_payload(),
    };

    let response = service
        .submit(request)
        .await
        .expect("submit should succeed");
    assert_eq!(
        response.status,
        RouteSubmissionStatus::Accepted,
        "the fixture service should accept a request that carries an idempotency key"
    );
}

#[test]
fn a_valid_coordinate_object_deserializes() {
    let location = parse_location(r#"{"lat": 51.5, "lng": -0.1}"#)
        .expect("a well-formed coordinate object should deserialize");
    let expected =
        RouteCoordinates::new(51.5, -0.1).expect("the fixture coordinates should validate");

    assert_eq!(
        location,
        RouteLocation::Coordinates(expected),
        "both coordinate fields should survive deserialization"
    );

    let at_the_limits = parse_location(r#"{"lat": 90.0, "lng": 180.0}"#)
        .expect("the WGS84 limits are inclusive and should deserialize");
    let limits = RouteCoordinates::new(90.0, 180.0).expect("the limit coordinates should validate");

    assert_eq!(
        at_the_limits,
        RouteLocation::Coordinates(limits),
        "the boundary values of both ranges should be accepted"
    );
}

/// Every way a coordinate object can be wrong, with the message naming it.
///
/// A repeated key and an unknown key are rejected rather than resolved, and a
/// missing or out-of-range field is refused, so a coordinate object can never
/// be silently ambiguous or outside WGS84.
#[rstest]
#[case::duplicate_lat(r#"{"lat": 1.0, "lat": 2.0, "lng": 3.0}"#, "duplicate field `lat`")]
#[case::duplicate_lng(r#"{"lat": 1.0, "lng": 2.0, "lng": 3.0}"#, "duplicate field `lng`")]
#[case::unknown_key(r#"{"lat": 1.0, "lng": 2.0, "zoom": 3}"#, "unknown field `zoom`")]
#[case::missing_lat(r#"{"lng": 2.0}"#, "missing field `lat`")]
#[case::missing_lng(r#"{"lat": 1.0}"#, "missing field `lng`")]
#[case::latitude_out_of_range(
    r#"{"lat": 90.5, "lng": 2.0}"#,
    "route latitude must be finite and within [-90, 90]"
)]
#[case::longitude_out_of_range(
    r#"{"lat": 1.0, "lng": 180.5}"#,
    "route longitude must be finite and within [-180, 180]"
)]
fn coordinate_objects_reject_invalid_fields(#[case] json: &str, #[case] expected: &str) {
    let error = parse_location(json)
        .expect_err("a coordinate object with this defect must not deserialize");
    assert!(
        error.to_string().contains(expected),
        "the deserializer error for `{json}` should name `{expected}`, got `{error}`"
    );
}
