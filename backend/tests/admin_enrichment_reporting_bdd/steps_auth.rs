//! Authentication-related admin enrichment reporting BDD steps.

use super::*;

#[given("a running server with session middleware")]
fn a_running_server_with_session_middleware(world: &WorldFixture) {
    bdd_common::setup_server(world);
}

#[given("the client has an authenticated session")]
fn the_client_has_an_authenticated_session(world: &WorldFixture) {
    bdd_common::setup_authenticated_session(world);
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

#[then("the response is unauthorized")]
fn the_response_is_unauthorized(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    assert_eq!(ctx.last_status, Some(401));
    assert!(ctx.enrichment_provenance.calls().is_empty());
}
