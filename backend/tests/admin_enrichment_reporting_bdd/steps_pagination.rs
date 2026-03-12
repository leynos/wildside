//! Pagination and payload admin enrichment reporting BDD steps.

use super::*;

fn assert_provenance_cursor_call(world: &WorldFixture, cursor_id: Uuid) {
    assert_single_provenance_call(
        world,
        ListEnrichmentProvenanceRequest::new(
            2,
            Some((fixture_timestamp("2026-02-28T12:00:00Z"), cursor_id)),
        ),
    );
}

#[given("persisted enrichment provenance reporting records exist")]
fn persisted_enrichment_provenance_reporting_records_exist(world: &WorldFixture) {
    world.world().borrow().enrichment_provenance.set_response(
        EnrichmentProvenanceListResponse::Ok(ListEnrichmentProvenanceResponse {
            records: vec![
                fixture_record(
                    "https://overpass.example/api/interpreter?seed=1",
                    fixture_timestamp("2026-02-28T12:00:00Z"),
                    [-3.2, 55.9, -3.0, 56.0],
                ),
                fixture_record(
                    "https://overpass.example/api/interpreter?seed=0",
                    fixture_timestamp("2026-02-28T12:00:00Z"),
                    [-3.3, 55.8, -3.1, 55.95],
                ),
            ],
            next_before: Some(fixture_cursor(
                fixture_timestamp("2026-02-28T12:00:00Z"),
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

#[when("the authenticated client requests enrichment provenance reporting with limit and cursor")]
fn the_authenticated_client_requests_enrichment_provenance_reporting_with_limit_and_cursor(
    world: &WorldFixture,
) {
    perform_provenance_request(world, "limit=2&before=2026-02-28T12:00:00Z");
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
    let expected_shared_imported_at = fixture_timestamp("2026-02-28T12:00:00Z");
    assert_record_payload(
        first,
        "https://overpass.example/api/interpreter?seed=1",
        expected_shared_imported_at,
        [-3.2, 55.9, -3.0, 56.0],
    );
    assert_record_payload(
        second,
        "https://overpass.example/api/interpreter?seed=0",
        expected_shared_imported_at,
        [-3.3, 55.8, -3.1, 55.95],
    );

    assert_records_sorted_newest_first(records);

    let expected_next_before = format!(
        "{}|{}",
        fixture_timestamp("2026-02-28T12:00:00Z").to_rfc3339(),
        Uuid::from_u128(0x11)
    );
    assert_eq!(
        body.get("nextBefore").and_then(Value::as_str),
        Some(expected_next_before.as_str())
    );
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
    assert_eq!(ctx.enrichment_provenance.calls().len(), 1);
}

#[then("the enrichment provenance query receives the expected limit and cursor")]
fn the_enrichment_provenance_query_receives_the_expected_limit_and_cursor(world: &WorldFixture) {
    assert_provenance_cursor_call(world, Uuid::max());
}

#[then("the enrichment provenance query receives the expected composite cursor")]
fn the_enrichment_provenance_query_receives_the_expected_composite_cursor(world: &WorldFixture) {
    assert_provenance_cursor_call(world, Uuid::from_u128(0x11));
}
