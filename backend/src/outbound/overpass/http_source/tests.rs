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

#[test]
fn an_empty_tag_list_yields_one_unfiltered_selector() {
    let selectors = collect_tag_selectors(&[]).expect("an empty tag list should be accepted");

    assert_eq!(
        selectors,
        vec![String::new()],
        "an empty tag list should still reach the whole bounding box, so exactly one \
         unfiltered selector is produced"
    );
}

#[test]
fn each_requested_tag_yields_its_own_selector() {
    let tags = vec!["amenity".to_owned(), "name=coffee".to_owned()];

    let selectors = collect_tag_selectors(&tags).expect("well-formed tags should be accepted");

    assert_eq!(
        selectors,
        vec![
            "[\"amenity\"]".to_owned(),
            "[\"name\"=\"coffee\"]".to_owned()
        ],
        "one selector per tag should be produced, in the order the tags were given"
    );
}

#[rstest]
#[case::blank_tag("   ", "tags must not include blank values")]
#[case::missing_key("=coffee", "tags must provide a non-empty key")]
#[case::missing_value("amenity=", "tags must not include empty values")]
fn an_invalid_tag_is_rejected(#[case] tag: &str, #[case] expected: &str) {
    let tags = vec![tag.to_owned()];

    let error = collect_tag_selectors(&tags).expect_err("an invalid tag should be rejected");

    assert!(
        is_invalid_request(&error),
        "`{tag}` should be refused as an invalid request, got {error:?}"
    );
    assert!(
        error.to_string().contains(expected),
        "the rejection of `{tag}` should explain `{expected}`, got `{error}`"
    );
}

#[test]
fn render_query_repeats_each_selector_for_every_element_type() {
    let selectors = vec!["[\"amenity\"=\"cafe\"]".to_owned()];

    let query = render_query(&selectors, "(1,2,3,4)", 25);

    assert_eq!(
        query,
        concat!(
            "[out:json][timeout:25];\n",
            "(\n",
            "  node[\"amenity\"=\"cafe\"](1,2,3,4);\n",
            "  way[\"amenity\"=\"cafe\"](1,2,3,4);\n",
            "  relation[\"amenity\"=\"cafe\"](1,2,3,4);\n",
            ");\n",
            "out center tags;"
        ),
        "each selector should be repeated for nodes, ways, and relations inside one union"
    );
}

#[test]
fn render_query_keeps_every_selector_and_the_configured_timeout() {
    let selectors = vec!["[\"amenity\"]".to_owned(), "[\"name\"=\"cafe\"]".to_owned()];

    let query = render_query(&selectors, "(1,2,3,4)", 90);

    assert!(
        query.starts_with("[out:json][timeout:90];"),
        "the configured timeout should reach the query header, got `{query}`"
    );
    assert_eq!(
        query.matches("(1,2,3,4);").count(),
        6,
        "two selectors across three element types should produce six clauses, got `{query}`"
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
