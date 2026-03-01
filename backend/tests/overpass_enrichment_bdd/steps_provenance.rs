//! Provenance persistence scenarios for Overpass enrichment BDD tests.

use super::*;
use backend::domain::ErrorCode;
use rstest_bdd_macros::{given, then};

#[given(
    "a Diesel-backed Overpass enrichment worker with successful source data and provenance capture"
)]
fn a_diesel_backed_overpass_enrichment_worker_with_successful_source_data_and_provenance_capture(
    world: &OverpassEnrichmentWorld,
) {
    let source_data = vec![Ok(world.make_response(2, 768))];
    world.setup_with_config_and_data(|_config| {}, source_data);
}

#[given("a Diesel-backed Overpass enrichment worker with unavailable provenance persistence")]
fn a_diesel_backed_overpass_enrichment_worker_with_unavailable_provenance_persistence(
    world: &OverpassEnrichmentWorld,
) {
    let source_data = vec![Ok(world.make_response(1, 256))];
    world.setup_with_config_and_data(|_config| {}, source_data);

    if world.skip_if_needed() {
        return;
    }
    world.drop_provenance_table();
}

#[given(
    "a Diesel-backed Overpass enrichment worker with successful zero-POI source data and provenance capture"
)]
fn a_diesel_backed_overpass_enrichment_worker_with_successful_zero_poi_source_data_and_provenance_capture(
    world: &OverpassEnrichmentWorld,
) {
    let source_data = vec![Ok(world.make_response(0, 64))];
    world.setup_with_config_and_data(|_config| {}, source_data);
}

#[then("enrichment provenance is persisted with source URL timestamp and bounding box")]
fn enrichment_provenance_is_persisted_with_source_url_timestamp_and_bounding_box(
    world: &OverpassEnrichmentWorld,
) {
    if world.skip_if_needed() {
        return;
    }

    let record = world
        .query_latest_provenance()
        .expect("query should run")
        .expect("one provenance row should exist");
    assert_eq!(record.0, "https://overpass.example/api/interpreter");
    assert_eq!(
        record.1, "2026-02-26T12:00:00Z",
        "imported_at should come from worker clock"
    );
    assert_eq!(record.2, LAUNCH_A_BOUNDS);
}

#[then("enrichment provenance write failures surface internal errors")]
fn enrichment_provenance_write_failures_surface_internal_errors(world: &OverpassEnrichmentWorld) {
    if world.skip_if_needed() {
        return;
    }

    let result = world.last_result.get().expect("last result should be set");
    let error = result.as_ref().expect_err("job should fail");
    assert_eq!(error.code(), ErrorCode::InternalError);
}

#[then("enrichment provenance entries are written even when zero POIs are returned")]
fn enrichment_provenance_entries_are_written_even_when_zero_pois_are_returned(
    world: &OverpassEnrichmentWorld,
) {
    if world.skip_if_needed() {
        return;
    }

    assert_eq!(world.query_poi_count().expect("poi count"), 0);
    assert_eq!(world.query_provenance_count().expect("provenance count"), 1);
}
