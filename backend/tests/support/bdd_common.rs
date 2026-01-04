//! Shared BDD helpers for PWA preferences and annotations tests.
//!
//! These helpers centralize common request patterns and assertions to keep the
//! step implementations concise and consistent.

use actix_web::http::Method;
use serde_json::Value;

use crate::harness::{AdapterWorld, WorldFixture};
use crate::pwa_http::{JsonRequest, login_and_store_cookie, perform_json_request};

/// Confirm the server is running for the scenario.
pub(super) fn setup_server(world: &WorldFixture) {
    let _ = world;
}

/// Establish an authenticated session and store the session cookie.
pub(super) fn setup_authenticated_session(world: &WorldFixture) {
    let shared_world = world.world();
    login_and_store_cookie(&shared_world);
}

/// Assert the last response returned HTTP 200.
pub(super) fn assert_response_ok(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    assert_eq!(ctx.last_status, Some(200));
}

/// Perform a GET request with the stored session cookie.
pub(super) fn perform_get_request(world: &WorldFixture, path: &str) {
    let shared_world = world.world();
    perform_json_request(
        &shared_world,
        JsonRequest {
            include_cookie: true,
            method: Method::GET,
            path,
            payload: None,
            idempotency_key: None,
        },
    );
}

/// Perform a mutation request with optional idempotency key.
pub(super) struct MutationRequest<'a> {
    pub(super) method: Method,
    pub(super) path: &'a str,
    pub(super) payload: Value,
    pub(super) idempotency_key: Option<&'a str>,
}

/// Perform a mutation request with optional idempotency key.
pub(super) fn perform_mutation_request(world: &WorldFixture, request: MutationRequest<'_>) {
    let shared_world = world.world();
    perform_json_request(
        &shared_world,
        JsonRequest {
            include_cookie: true,
            method: request.method,
            path: request.path,
            payload: Some(request.payload),
            idempotency_key: request.idempotency_key,
        },
    );
}

/// Assert that the captured idempotency key matches the expected value.
pub(super) fn assert_idempotency_key_captured<F>(
    world: &WorldFixture,
    get_idempotency_key: F,
    expected_key: &str,
) where
    F: FnOnce(&AdapterWorld) -> Option<String>,
{
    let ctx = world.world();
    let ctx = ctx.borrow();
    let idempotency_key = get_idempotency_key(&ctx).expect("idempotency key");
    assert_eq!(idempotency_key, expected_key);
}
