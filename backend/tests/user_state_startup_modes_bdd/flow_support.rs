//! Shared flow helpers for user-state startup-mode BDD tests.

use std::sync::Arc;

use actix_web::web;
use backend::domain::ports::RouteSubmissionService;
use backend::inbound::http::state::HttpState;
use backend::test_support::server::{ServerConfig, build_http_state};
use serde_json::Value;

pub(crate) use crate::support::flow_helpers::{parse_json_body, run_async};

const FIXTURE_USERS_ID: &str = "3fa85f64-5717-4562-b3fc-2c963f66afa6";
const FIXTURE_USERS_NAME: &str = "Ada Lovelace";

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
