//! Shared flow helpers for user-state startup-mode BDD tests.

use std::future::Future;
use std::sync::Arc;

use actix_web::web;
use backend::domain::ports::RouteSubmissionService;
use backend::inbound::http::state::HttpState;
use backend::test_support::server::{ServerConfig, build_http_state};
use serde_json::Value;

const FIXTURE_USERS_ID: &str = "3fa85f64-5717-4562-b3fc-2c963f66afa6";
const FIXTURE_USERS_NAME: &str = "Ada Lovelace";

/// Drives an async future to completion on a fresh single-threaded runtime.
///
/// A new Tokio runtime is created per call so each step runs in isolation
/// without sharing an executor.
///
/// # Parameters
///
/// - `future`: the future to run to completion.
///
/// # Returns
///
/// The value the future resolves to.
pub(crate) fn run_async<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(future)
}

/// Builds the HTTP application state used by the startup-mode BDD tests.
///
/// # Parameters
///
/// - `config`: server configuration describing the wiring under test.
/// - `route_submission`: the route-submission service to inject into the state.
///
/// # Returns
///
/// The assembled [`HttpState`], wrapped in [`web::Data`] for Actix handlers.
pub(crate) fn build_http_state_for_tests(
    config: &ServerConfig,
    route_submission: Arc<dyn RouteSubmissionService>,
) -> web::Data<HttpState> {
    build_http_state(config, route_submission)
}

/// Reports whether a users-list response contains the seeded fixture user.
///
/// # Parameters
///
/// - `body`: the parsed JSON response body; its `data` array is inspected.
///
/// # Returns
///
/// `true` when any entry matches the fixture user's id and display name.
///
/// # Panics
///
/// Panics if `body` has no `data` array.
pub(crate) fn is_fixture_users(body: &Value) -> bool {
    let users = body
        .get("data")
        .and_then(Value::as_array)
        .expect("users data array");
    users.iter().any(|user| {
        user.get("id").and_then(Value::as_str) == Some(FIXTURE_USERS_ID)
            && user.get("displayName").and_then(Value::as_str) == Some(FIXTURE_USERS_NAME)
    })
}

/// Parses an HTTP response body as JSON, treating an empty body as absent.
///
/// # Parameters
///
/// - `bytes`: the raw response body.
///
/// # Returns
///
/// `None` when `bytes` is empty, otherwise `Some(value)` with the parsed JSON.
///
/// # Panics
///
/// Panics if a non-empty `bytes` is not valid JSON.
pub(crate) fn parse_json_body(bytes: &[u8]) -> Option<Value> {
    if bytes.is_empty() {
        None
    } else {
        Some(serde_json::from_slice(bytes).expect("json body"))
    }
}
