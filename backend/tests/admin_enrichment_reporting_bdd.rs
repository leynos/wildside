//! Behavioural tests for admin enrichment provenance reporting endpoint.

// Shared test doubles include helpers unused in this specific crate.
#[expect(
    clippy::type_complexity,
    reason = "Shared test doubles include helpers unused in this specific crate."
)]
#[expect(
    dead_code,
    reason = "Shared test doubles include helpers unused in this specific crate."
)]
#[path = "adapter_guardrails/doubles.rs"]
mod doubles;
// Shared helpers include functions used only by other integration suites.
#[expect(
    dead_code,
    reason = "Shared helpers include functions used only by other integration suites."
)]
#[path = "support/bdd_common.rs"]
mod bdd_common;
#[expect(
    dead_code,
    reason = "Shared harness has extra fields used by other integration suites."
)]
#[path = "adapter_guardrails/harness.rs"]
mod harness;
#[path = "support/pwa_http.rs"]
mod pwa_http;
#[path = "support/ws.rs"]
mod ws_support;

use actix_web::http::Method;
use backend::domain::ports::{
    EnrichmentProvenanceCursor, EnrichmentProvenanceRecord, EnrichmentProvenanceRepositoryError,
    ListEnrichmentProvenanceRequest, ListEnrichmentProvenanceResponse,
};
use chrono::{DateTime, Utc};
use doubles::EnrichmentProvenanceListResponse;
use harness::WorldFixture;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use serde_json::Value;
use uuid::Uuid;

const ENDPOINT: &str = "/api/v1/admin/enrichment/provenance";

#[fixture]
fn world() -> WorldFixture {
    harness::world()
}

fn fixture_timestamp(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .expect("valid fixture timestamp")
        .with_timezone(&Utc)
}

fn fixture_record(
    source_url: &str,
    imported_at: &str,
    bounding_box: [f64; 4],
) -> EnrichmentProvenanceRecord {
    EnrichmentProvenanceRecord {
        source_url: source_url.to_owned(),
        imported_at: fixture_timestamp(imported_at),
        bounding_box,
    }
}

fn fixture_cursor(imported_at: &str, id: Uuid) -> EnrichmentProvenanceCursor {
    EnrichmentProvenanceCursor::new(fixture_timestamp(imported_at), id)
}

fn assert_record_payload(
    record: &Value,
    expected_source_url: &str,
    expected_imported_at: &str,
    expected_bounding_box: [f64; 4],
) {
    assert_eq!(
        record.get("sourceUrl").and_then(Value::as_str),
        Some(expected_source_url)
    );
    assert_eq!(
        record.get("importedAt").and_then(Value::as_str),
        Some(expected_imported_at)
    );

    let bounding = record
        .get("boundingBox")
        .and_then(Value::as_object)
        .expect("boundingBox object");
    assert_eq!(
        bounding.get("minLng").and_then(Value::as_f64),
        Some(expected_bounding_box[0])
    );
    assert_eq!(
        bounding.get("minLat").and_then(Value::as_f64),
        Some(expected_bounding_box[1])
    );
    assert_eq!(
        bounding.get("maxLng").and_then(Value::as_f64),
        Some(expected_bounding_box[2])
    );
    assert_eq!(
        bounding.get("maxLat").and_then(Value::as_f64),
        Some(expected_bounding_box[3])
    );
}

fn assert_records_sorted_newest_first(records: &[Value]) {
    let imported_at = records
        .iter()
        .map(|record| {
            record
                .get("importedAt")
                .and_then(Value::as_str)
                .expect("importedAt")
        })
        .map(|value| DateTime::parse_from_rfc3339(value).expect("record importedAt RFC3339"))
        .collect::<Vec<_>>();

    assert!(
        imported_at.windows(2).all(|pair| pair[0] >= pair[1]),
        "expected records to be sorted newest-first by importedAt"
    );
}

fn perform_provenance_request(world: &WorldFixture, query: &str) {
    bdd_common::perform_get_request(world, &format!("{}?{}", ENDPOINT, query));
}

fn assert_single_provenance_call(world: &WorldFixture, expected: ListEnrichmentProvenanceRequest) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    let calls = ctx.enrichment_provenance.calls();
    assert_eq!(
        calls.len(),
        1,
        "expected exactly one provenance repository call"
    );
    assert_eq!(calls[0], expected);
}

#[given("a running server with session middleware")]
fn a_running_server_with_session_middleware(world: &WorldFixture) {
    bdd_common::setup_server(world);
}

#[given("the client has an authenticated session")]
fn the_client_has_an_authenticated_session(world: &WorldFixture) {
    bdd_common::setup_authenticated_session(world);
}

#[given("persisted enrichment provenance reporting records exist")]
fn persisted_enrichment_provenance_reporting_records_exist(world: &WorldFixture) {
    world.world().borrow().enrichment_provenance.set_response(
        EnrichmentProvenanceListResponse::Ok(ListEnrichmentProvenanceResponse {
            records: vec![
                fixture_record(
                    "https://overpass.example/api/interpreter?seed=1",
                    "2026-02-28T12:00:00Z",
                    [-3.2, 55.9, -3.0, 56.0],
                ),
                fixture_record(
                    "https://overpass.example/api/interpreter?seed=0",
                    "2026-02-28T12:00:00Z",
                    [-3.3, 55.8, -3.1, 55.95],
                ),
            ],
            next_before: Some(fixture_cursor(
                "2026-02-28T12:00:00Z",
                Uuid::from_u128(0x11),
            )),
        }),
    );
}

#[given("no enrichment provenance reporting records exist")]
fn no_enrichment_provenance_reporting_records_exist(world: &WorldFixture) {
    world.world().borrow().enrichment_provenance.set_response(
        EnrichmentProvenanceListResponse::Ok(ListEnrichmentProvenanceResponse {
            records: Vec::new(),
            next_before: None,
        }),
    );
}

#[given("enrichment provenance reporting is unavailable")]
fn enrichment_provenance_reporting_is_unavailable(world: &WorldFixture) {
    world.world().borrow().enrichment_provenance.set_response(
        EnrichmentProvenanceListResponse::Err(EnrichmentProvenanceRepositoryError::connection(
            "reporting store offline",
        )),
    );
}

#[when("the authenticated client requests enrichment provenance reporting")]
fn the_authenticated_client_requests_enrichment_provenance_reporting(world: &WorldFixture) {
    bdd_common::perform_get_request(world, ENDPOINT);
}

#[when("the unauthenticated client requests enrichment provenance reporting")]
fn the_unauthenticated_client_requests_enrichment_provenance_reporting(world: &WorldFixture) {
    let shared_world = world.world();
    pwa_http::perform_json_request(
        &shared_world,
        pwa_http::JsonRequest {
            include_cookie: false,
            method: Method::GET,
            path: ENDPOINT,
            payload: None,
            idempotency_key: None,
        },
    );
}

#[when("the authenticated client requests enrichment provenance reporting with invalid limit")]
fn the_authenticated_client_requests_enrichment_provenance_reporting_with_invalid_limit(
    world: &WorldFixture,
) {
    perform_provenance_request(world, "limit=0");
}

#[when("the authenticated client requests enrichment provenance reporting with invalid cursor")]
fn the_authenticated_client_requests_enrichment_provenance_reporting_with_invalid_cursor(
    world: &WorldFixture,
) {
    perform_provenance_request(world, "before=not-a-timestamp");
}

#[when("the authenticated client requests enrichment provenance reporting with a composite cursor")]
fn the_authenticated_client_requests_enrichment_provenance_reporting_with_a_composite_cursor(
    world: &WorldFixture,
) {
    perform_provenance_request(
        world,
        &format!(
            "limit=2&before=2026-02-28T12:00:00Z|{}",
            Uuid::from_u128(0x11)
        ),
    );
}

#[when(
    "the authenticated client requests enrichment provenance reporting with a malformed composite cursor"
)]
fn the_authenticated_client_requests_enrichment_provenance_reporting_with_a_malformed_composite_cursor(
    world: &WorldFixture,
) {
    perform_provenance_request(world, "limit=2&before=2026-02-28T12:00:00Z|not-a-uuid");
}

#[when("the authenticated client requests enrichment provenance reporting with over-max limit")]
fn the_authenticated_client_requests_enrichment_provenance_reporting_with_over_max_limit(
    world: &WorldFixture,
) {
    perform_provenance_request(world, "limit=201");
}

#[when("the authenticated client requests enrichment provenance reporting with limit and cursor")]
fn the_authenticated_client_requests_enrichment_provenance_reporting_with_limit_and_cursor(
    world: &WorldFixture,
) {
    perform_provenance_request(world, "limit=2&before=2026-02-28T12:00:00Z");
}

#[then("the response is ok with an enrichment provenance payload")]
fn the_response_is_ok_with_an_enrichment_provenance_payload(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    assert_eq!(ctx.last_status, Some(200));

    let body = ctx.last_body.as_ref().expect("response body");
    let records = body
        .get("records")
        .and_then(Value::as_array)
        .expect("records array");
    assert_eq!(records.len(), 2);

    let first = records.first().expect("first record");
    let second = records.get(1).expect("second record");
    let expected_shared_imported_at = fixture_timestamp("2026-02-28T12:00:00Z").to_rfc3339();
    assert_record_payload(
        first,
        "https://overpass.example/api/interpreter?seed=1",
        &expected_shared_imported_at,
        [-3.2, 55.9, -3.0, 56.0],
    );
    assert_record_payload(
        second,
        "https://overpass.example/api/interpreter?seed=0",
        &expected_shared_imported_at,
        [-3.3, 55.8, -3.1, 55.95],
    );

    assert_records_sorted_newest_first(records);

    let expected_next_before = format!("{}|{}", expected_shared_imported_at, Uuid::from_u128(0x11));
    assert_eq!(
        body.get("nextBefore").and_then(Value::as_str),
        Some(expected_next_before.as_str())
    );
}

#[then("the response is unauthorized")]
fn the_response_is_unauthorized(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    assert_eq!(ctx.last_status, Some(401));
    assert!(ctx.enrichment_provenance.calls().is_empty());
}

#[then("the response is bad request")]
fn the_response_is_bad_request(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    assert_eq!(ctx.last_status, Some(400));
    assert!(ctx.enrichment_provenance.calls().is_empty());
}

#[then("the response is ok with an empty enrichment provenance payload")]
fn the_response_is_ok_with_an_empty_enrichment_provenance_payload(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    assert_eq!(ctx.last_status, Some(200));

    let body = ctx.last_body.as_ref().expect("response body");
    let records = body
        .get("records")
        .and_then(Value::as_array)
        .expect("records array");
    assert!(records.is_empty(), "expected empty records array");
    let next_before = body.get("nextBefore");
    assert!(
        next_before.is_none() || next_before == Some(&Value::Null),
        "expected nextBefore to be absent or null for empty payload, got {next_before:?}"
    );
}

#[then("the response includes a nextBefore cursor")]
fn the_response_includes_a_next_before_cursor(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    assert_eq!(ctx.last_status, Some(200));

    let body = ctx.last_body.as_ref().expect("response body");
    assert!(
        body.get("nextBefore").and_then(Value::as_str).is_some(),
        "nextBefore should be present"
    );
}

#[then("the response is service unavailable")]
fn the_response_is_service_unavailable(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    assert_eq!(ctx.last_status, Some(503));
}

#[then("the enrichment provenance query receives the expected limit and cursor")]
fn the_enrichment_provenance_query_receives_the_expected_limit_and_cursor(world: &WorldFixture) {
    assert_single_provenance_call(
        world,
        ListEnrichmentProvenanceRequest::new(
            2,
            Some((fixture_timestamp("2026-02-28T12:00:00Z"), Uuid::max())),
        ),
    );
}

#[then("the enrichment provenance query receives the expected composite cursor")]
fn the_enrichment_provenance_query_receives_the_expected_composite_cursor(world: &WorldFixture) {
    assert_single_provenance_call(
        world,
        ListEnrichmentProvenanceRequest::new(
            2,
            Some((
                fixture_timestamp("2026-02-28T12:00:00Z"),
                Uuid::from_u128(0x11),
            )),
        ),
    );
}

#[scenario(path = "tests/features/admin_enrichment_reporting.feature")]
fn admin_enrichment_reporting(world: WorldFixture) {
    drop(world);
}
