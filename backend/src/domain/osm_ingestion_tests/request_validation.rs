//! Unit coverage for request validation and geofence predicates.

use rstest::rstest;

use super::*;
use crate::domain::ErrorCode;

#[rstest]
#[case::inside(-3.20, 55.95, true)]
#[case::on_min_boundary(-3.30, 55.90, true)]
#[case::on_max_boundary(-3.10, 56.00, true)]
#[case::outside_longitude(-3.31, 55.95, false)]
#[case::outside_latitude(-3.20, 56.01, false)]
#[case::nan_coordinate(-3.20, f64::NAN, false)]
fn geofence_contains_includes_boundaries_and_rejects_non_finite(
    #[case] longitude: f64,
    #[case] latitude: f64,
    #[case] expected: bool,
) {
    let bounds = GeofenceBounds::new(
        GEOFENCE_BOUNDS[0],
        GEOFENCE_BOUNDS[1],
        GEOFENCE_BOUNDS[2],
        GEOFENCE_BOUNDS[3],
    )
    .expect("fixture bounds should be valid");
    let actual = bounds.contains(longitude, latitude);
    assert_eq!(actual, expected);
}

#[rstest]
#[case::blank_source_url("", "launch-a", INPUT_DIGEST, "sourceUrl must not be empty")]
#[case::invalid_source_url("not-a-url", "launch-a", INPUT_DIGEST, "sourceUrl must be a valid URL")]
#[case::blank_geofence_id(SOURCE_URL, " ", INPUT_DIGEST, "geofenceId must not be empty")]
#[case::invalid_digest_length(
    SOURCE_URL,
    "launch-a",
    "deadbeef",
    "inputDigest must be a 64-character lowercase hexadecimal SHA-256 digest"
)]
#[case::invalid_digest_uppercase(
    SOURCE_URL,
    "launch-a",
    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    "inputDigest must be a 64-character lowercase hexadecimal SHA-256 digest"
)]
fn validate_request_rejects_invalid_fields(
    #[case] source_url: &str,
    #[case] geofence_id: &str,
    #[case] input_digest: &str,
    #[case] expected_message: &str,
) {
    let request = OsmIngestionRequest {
        source_url: source_url.to_owned(),
        geofence_id: geofence_id.to_owned(),
        input_digest: input_digest.to_owned(),
        ..request()
    };

    let error = validate_request(&request).expect_err("invalid request should fail");
    assert_eq!(error.code(), ErrorCode::InvalidRequest);
    assert_eq!(error.message(), expected_message);
}

#[rstest]
fn validate_request_trims_geofence_id() {
    let request = OsmIngestionRequest {
        geofence_id: "  launch-a  ".to_owned(),
        ..request()
    };

    let validated = validate_request(&request).expect("request should be valid");
    assert_eq!(validated.geofence_id.as_str(), "launch-a");
}

#[rstest]
fn validate_request_trims_source_url() {
    let request = OsmIngestionRequest {
        source_url: "  https://example.test/launch.osm.pbf  ".to_owned(),
        ..request()
    };

    let validated = validate_request(&request).expect("request should be valid");
    assert_eq!(
        validated.source_url.as_str(),
        "https://example.test/launch.osm.pbf"
    );
}

#[rstest]
#[case::nan_longitude(
    [f64::NAN, 55.90, -3.10, 56.00],
    "geofence longitude values must be finite and within [-180, 180]"
)]
#[case::infinite_latitude(
    [-3.30, f64::INFINITY, -3.10, 56.00],
    "geofence latitude values must be finite and within [-90, 90]"
)]
#[case::negative_infinite_latitude(
    [-3.30, f64::NEG_INFINITY, -3.10, 56.00],
    "geofence latitude values must be finite and within [-90, 90]"
)]
#[case::longitude_out_of_range(
    [-181.0, 55.90, -3.10, 56.00],
    "geofence longitude values must be finite and within [-180, 180]"
)]
#[case::latitude_out_of_range(
    [-3.30, -91.0, -3.10, 56.00],
    "geofence latitude values must be finite and within [-90, 90]"
)]
#[case::inverted_longitude(
    [-3.00, 55.90, -3.10, 56.00],
    "geofenceBounds must be ordered as [minLng, minLat, maxLng, maxLat]"
)]
#[case::inverted_latitude(
    [-3.30, 56.10, -3.10, 56.00],
    "geofenceBounds must be ordered as [minLng, minLat, maxLng, maxLat]"
)]
fn validate_request_rejects_invalid_bounds(
    #[case] geofence_bounds: [f64; 4],
    #[case] expected_message: &str,
) {
    let request = OsmIngestionRequest {
        geofence_bounds,
        ..request()
    };

    let error = validate_request(&request).expect_err("invalid bounds should fail");
    assert_eq!(error.code(), ErrorCode::InvalidRequest);
    assert_eq!(error.message(), expected_message);
}
