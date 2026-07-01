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

pub(crate) fn run_async<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(future)
}

pub(crate) fn build_http_state_for_tests(
    config: &ServerConfig,
    route_submission: Arc<dyn RouteSubmissionService>,
) -> web::Data<HttpState> {
    build_http_state(config, route_submission)
}

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

pub(crate) fn parse_json_body(bytes: &[u8]) -> Option<Value> {
    if bytes.is_empty() {
        None
    } else {
        Some(serde_json::from_slice(bytes).expect("json body"))
    }
}
