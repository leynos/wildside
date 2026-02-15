//! Behavioural tests for catalogue and descriptor read endpoints.
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
// Shared harness has extra fields used by other integration suites.
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

use backend::domain::ports::{CatalogueRepositoryError, DescriptorRepositoryError};
use doubles::{CatalogueQueryResponse, DescriptorQueryResponse};
use harness::WorldFixture;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use serde_json::Value;

const EXPLORE_PATH: &str = "/api/v1/catalogue/explore";
const DESCRIPTORS_PATH: &str = "/api/v1/catalogue/descriptors";

#[fixture]
fn world() -> WorldFixture {
    harness::world()
}

#[given("a running server with session middleware")]
fn a_running_server_with_session_middleware(world: &WorldFixture) {
    bdd_common::setup_server(world);
}

#[given("the client has an authenticated session")]
fn the_client_has_an_authenticated_session(world: &WorldFixture) {
    bdd_common::setup_authenticated_session(world);
}

#[given("the catalogue repository returns a connection error")]
fn the_catalogue_repository_returns_a_connection_error(world: &WorldFixture) {
    world
        .world()
        .borrow()
        .catalogue
        .set_response(CatalogueQueryResponse::Err(
            CatalogueRepositoryError::connection("test connection failure".to_string()),
        ));
}

#[given("the descriptor repository returns a connection error")]
fn the_descriptor_repository_returns_a_connection_error(world: &WorldFixture) {
    world
        .world()
        .borrow()
        .descriptors
        .set_response(DescriptorQueryResponse::Err(
            DescriptorRepositoryError::connection("test connection failure".to_string()),
        ));
}

#[when("the client requests the explore catalogue")]
fn the_client_requests_the_explore_catalogue(world: &WorldFixture) {
    bdd_common::perform_get_request(world, EXPLORE_PATH);
}

#[when("the client requests the descriptors")]
fn the_client_requests_the_descriptors(world: &WorldFixture) {
    bdd_common::perform_get_request(world, DESCRIPTORS_PATH);
}

#[when("the client requests the explore catalogue without a session")]
fn the_client_requests_the_explore_catalogue_without_a_session(world: &WorldFixture) {
    let shared = world.world();
    pwa_http::perform_json_request(
        &shared,
        pwa_http::JsonRequest {
            include_cookie: false,
            method: actix_web::http::Method::GET,
            path: EXPLORE_PATH,
            payload: None,
            idempotency_key: None,
        },
    );
}

#[when("the client requests the descriptors without a session")]
fn the_client_requests_the_descriptors_without_a_session(world: &WorldFixture) {
    let shared = world.world();
    pwa_http::perform_json_request(
        &shared,
        pwa_http::JsonRequest {
            include_cookie: false,
            method: actix_web::http::Method::GET,
            path: DESCRIPTORS_PATH,
            payload: None,
            idempotency_key: None,
        },
    );
}

#[then("the response is ok")]
fn the_response_is_ok(world: &WorldFixture) {
    bdd_common::assert_response_ok(world);
}

#[then("the response is unauthorised")]
fn the_response_is_unauthorised(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    assert_eq!(ctx.last_status, Some(401));
}

#[then("the response is service unavailable")]
fn the_response_is_service_unavailable(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    assert_eq!(ctx.last_status, Some(503));
}

#[then("the response includes a generated_at timestamp")]
fn the_response_includes_a_generated_at_timestamp(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    let body = ctx.last_body.as_ref().expect("response body");
    let generated_at = body
        .get("generatedAt")
        .and_then(Value::as_str)
        .expect("generatedAt field");
    assert!(
        generated_at.contains('T'),
        "generatedAt should be an RFC 3339 timestamp"
    );
}

#[then("the explore response includes empty arrays for all collections")]
fn the_explore_response_includes_empty_arrays_for_all_collections(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    let body = ctx.last_body.as_ref().expect("response body");
    for field in ["categories", "routes", "themes", "collections", "trending"] {
        let arr = body.get(field).and_then(Value::as_array);
        assert!(
            arr.map(|a| a.is_empty()).unwrap_or(false),
            "{field} should be an empty array"
        );
    }
    assert!(
        body.get("communityPick").is_some(),
        "communityPick field should be present"
    );
}

#[then("the descriptors response includes empty arrays for all registries")]
fn the_descriptors_response_includes_empty_arrays_for_all_registries(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    let body = ctx.last_body.as_ref().expect("response body");
    for field in [
        "tags",
        "badges",
        "safetyToggles",
        "safetyPresets",
        "interestThemes",
    ] {
        let arr = body.get(field).and_then(Value::as_array);
        assert!(
            arr.map(|a| a.is_empty()).unwrap_or(false),
            "{field} should be an empty array"
        );
    }
}

#[scenario(path = "tests/features/catalogue_endpoints.feature")]
fn catalogue_endpoints(world: WorldFixture) {
    drop(world);
}
