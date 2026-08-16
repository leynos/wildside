//! Regression coverage for Overpass HTTP transport and mapping helpers.

use rstest::rstest;

use super::*;
use crate::domain::BoundingBox;
use crate::test_support::overpass_enrichment::enrichment_request;

#[test]
fn builds_query_with_bbox_reordered_for_overpass() {
    let query = build_overpass_query(
        &enrichment_request(vec!["amenity", "name=coffee \"bar\""]),
        180,
    )
    .expect("query should build");

    assert!(
        query.contains("node[\"amenity\"](55.9,-3.3,56,-3.1);"),
        "query should include bbox in south,west,north,east order"
    );
    assert!(
        query.starts_with("[out:json][timeout:180];"),
        "query should include configured timeout"
    );
    assert!(
        query.contains("way[\"name\"=\"coffee \\\"bar\\\"\"](55.9,-3.3,56,-3.1);"),
        "query should escape quoted values in tag selectors"
    );
}

#[rstest]
#[case::rate_limited(
    StatusCode::TOO_MANY_REQUESTS,
    is_rate_limited,
    "429 should map to RateLimited"
)]
#[case::request_timeout(
    StatusCode::REQUEST_TIMEOUT,
    is_timeout,
    "timeout statuses should map to Timeout"
)]
#[case::gateway_timeout(
    StatusCode::GATEWAY_TIMEOUT,
    is_timeout,
    "timeout statuses should map to Timeout"
)]
#[case::bad_request(
    StatusCode::BAD_REQUEST,
    is_invalid_request,
    "client statuses should map to InvalidRequest"
)]
#[case::server_error(
    StatusCode::INTERNAL_SERVER_ERROR,
    is_transport,
    "other statuses should map to Transport"
)]
fn maps_http_statuses_to_expected_domain_errors(
    #[case] status: StatusCode,
    #[case] expected: fn(&EnrichmentSourceError) -> bool,
    #[case] message: &str,
) {
    let error = map_status_error(status, b"{\"remark\":\"backend unavailable\"}");
    assert!(expected(&error), "{message}");
}

fn is_rate_limited(error: &EnrichmentSourceError) -> bool {
    matches!(error, EnrichmentSourceError::RateLimited { .. })
}

fn is_timeout(error: &EnrichmentSourceError) -> bool {
    matches!(error, EnrichmentSourceError::Timeout { .. })
}

fn is_invalid_request(error: &EnrichmentSourceError) -> bool {
    matches!(error, EnrichmentSourceError::InvalidRequest { .. })
}

fn is_transport(error: &EnrichmentSourceError) -> bool {
    matches!(error, EnrichmentSourceError::Transport { .. })
}

#[test]
fn parses_overpass_json_into_domain_pois() {
    let body = r#"{
        "elements": [
            {
                "type": "node",
                "id": 101,
                "lat": 55.91,
                "lon": -3.21,
                "tags": { "amenity": "cafe" }
            },
            {
                "type": "way",
                "id": 102,
                "center": { "lat": 55.92, "lon": -3.22 },
                "tags": { "name": "The Meadows" }
            }
        ]
    }"#;

    let pois = parse_pois(body.as_bytes()).expect("JSON should decode");
    assert_eq!(pois.len(), 2, "two POIs should be decoded");
    assert_eq!(pois[0].element_type, "node");
    assert_eq!(pois[0].longitude, -3.21);
    assert_eq!(pois[1].element_type, "way");
    assert_eq!(pois[1].latitude, 55.92);
}

#[test]
fn rejects_elements_without_coordinates() {
    let body = r#"{
        "elements": [
            { "type": "way", "id": 201, "tags": { "name": "missing-centre" } }
        ]
    }"#;

    let error = parse_pois(body.as_bytes()).expect_err("decode should fail");
    assert!(
        matches!(error, EnrichmentSourceError::Decode { .. }),
        "missing coordinates should map to Decode errors",
    );
}

#[rstest]
#[case::longitude_out_of_range(-181.0, 55.90, -3.10, 56.00)]
#[case::latitude_out_of_range(-3.30, -91.0, -3.10, 56.00)]
fn bbox_outside_wgs84_ranges_cannot_reach_the_adapter(
    #[case] min_lng: f64,
    #[case] min_lat: f64,
    #[case] max_lng: f64,
    #[case] max_lat: f64,
) {
    // `BoundingBox` owns WGS84 validity, so an out-of-range box is
    // unrepresentable in `EnrichmentRequest` and never reaches query
    // building. Range-by-range coverage lives with `BoundingBox` itself.
    assert!(
        BoundingBox::new(min_lng, min_lat, max_lng, max_lat).is_err(),
        "out-of-range coordinates must be rejected before an \
         EnrichmentRequest can be constructed",
    );
}
