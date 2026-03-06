//! Validation-focused admin enrichment reporting BDD steps.

use super::*;

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

#[when("the authenticated client requests enrichment provenance reporting with over-max limit")]
fn the_authenticated_client_requests_enrichment_provenance_reporting_with_over_max_limit(
    world: &WorldFixture,
) {
    perform_provenance_request(world, "limit=201");
}

#[when(
    "the authenticated client requests enrichment provenance reporting with a malformed composite cursor"
)]
fn the_authenticated_client_requests_enrichment_provenance_reporting_with_a_malformed_composite_cursor(
    world: &WorldFixture,
) {
    perform_provenance_request(world, "limit=2&before=2026-02-28T12:00:00Z|not-a-uuid");
}

#[then("the response is bad request")]
fn the_response_is_bad_request(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    assert_eq!(ctx.last_status, Some(400));
    assert!(ctx.enrichment_provenance.calls().is_empty());
}
