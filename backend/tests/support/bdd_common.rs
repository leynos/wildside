//! Shared BDD helpers for PWA preferences and annotations tests.
//!
//! These helpers centralize common request patterns and assertions to keep the
//! step implementations concise and consistent.

use actix_web::http::Method;
use backend::domain::Error;
use serde_json::Value;

use crate::harness::{AdapterWorld, WorldFixture};
use crate::pwa_http::{JsonRequest, login_and_store_cookie, perform_json_request};

// -- Test error constructors --

/// Creates a revision mismatch conflict error with structured details.
pub(super) fn revision_mismatch_error(expected: u32, actual: u32) -> Error {
    Error::conflict("revision mismatch").with_details(serde_json::json!({
        "expectedRevision": expected,
        "actualRevision": actual
    }))
}

/// Creates an idempotency conflict error.
pub(super) fn idempotency_conflict_error() -> Error {
    Error::conflict("idempotency key already used with different payload")
}

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

// -- Conflict response assertions --

/// Assert the response is a 409 conflict with revision mismatch details.
pub(super) fn assert_conflict_with_revision_details(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    assert_eq!(ctx.last_status, Some(409));
    let body = ctx.last_body.as_ref().expect("response body");
    assert_eq!(body.get("code").and_then(Value::as_str), Some("conflict"));
    let details = body.get("details").expect("details should be present");
    assert!(
        details.get("expectedRevision").is_some(),
        "expectedRevision should be present in details"
    );
    assert!(
        details.get("actualRevision").is_some(),
        "actualRevision should be present in details"
    );
}

/// Assert the response is a 409 conflict with idempotency message.
pub(super) fn assert_conflict_with_idempotency_message(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    assert_eq!(ctx.last_status, Some(409));
    let body = ctx.last_body.as_ref().expect("response body");
    assert_eq!(body.get("code").and_then(Value::as_str), Some("conflict"));
    let message = body
        .get("message")
        .and_then(Value::as_str)
        .expect("message");
    assert!(
        message.contains("idempotency"),
        "message should mention idempotency"
    );
}

/// Assert the response body includes `replayed: true`.
pub(super) fn assert_response_replayed(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    let body = ctx.last_body.as_ref().expect("response body");
    assert_eq!(body.get("replayed").and_then(Value::as_bool), Some(true));
}
