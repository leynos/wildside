//! Behaviour tests for session-enforced user endpoints.
//!
//! These scenarios confirm that `/api/v1/users/me` and
//! `/api/v1/users/me/interests` require authenticated sessions and return trace
//! identifiers on unauthorised responses.
//
// rstest-bdd generates guard variables with double underscores, which trips
// the non_snake_case lint under -D warnings.
#![allow(non_snake_case)]

// Shared test doubles include helpers unused in this specific crate.
#[allow(dead_code, clippy::type_complexity)]
#[path = "adapter_guardrails/doubles.rs"]
mod doubles;
// Shared harness has extra fields used by other integration suites.
#[allow(dead_code)]
#[path = "adapter_guardrails/harness.rs"]
mod harness;
#[path = "support/ws.rs"]
mod ws_support;

use actix_web::http::{header, Method};
use awc::Client;
use backend::domain::TRACE_ID_HEADER;
use harness::{with_world_async, WorldFixture};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use serde_json::Value;

use crate::harness::SharedWorld;

#[fixture]
fn world() -> WorldFixture {
    harness::world()
}

fn record_response(world: &SharedWorld, status: u16, trace_id: Option<String>, body: Value) {
    let mut ctx = world.borrow_mut();
    ctx.last_status = Some(status);
    ctx.last_trace_id = trace_id;
    ctx.last_body = Some(body);
}

fn session_cookie(world: &SharedWorld) -> String {
    world
        .borrow()
        .session_cookie
        .clone()
        .expect("session cookie")
        .split(';')
        .next()
        .expect("cookie pair")
        .to_owned()
}

fn login_and_store_cookie(world: &SharedWorld) {
    let (status, cookie_header) = with_world_async(world, |base_url| async move {
        let response = Client::default()
            .post(format!("{base_url}/api/v1/login"))
            .send_json(&serde_json::json!({
                "username": "admin",
                "password": "password"
            }))
            .await
            .expect("login request");

        let status = response.status().as_u16();
        let cookie_header = response
            .headers()
            .get(header::SET_COOKIE)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_owned());
        (status, cookie_header)
    });

    let mut ctx = world.borrow_mut();
    ctx.last_status = Some(status);
    ctx.session_cookie = cookie_header;
    ctx.last_trace_id = None;
    ctx.last_body = None;
}

struct RequestSpec<'a> {
    method: Method,
    path: &'a str,
    payload: Option<Value>,
    label: &'a str,
}

fn perform_json_request(world: &SharedWorld, include_cookie: bool, spec: RequestSpec<'_>) {
    let RequestSpec {
        method,
        path,
        payload,
        label,
    } = spec;
    let cookie = include_cookie.then(|| session_cookie(world));
    let (status, trace_id, body) = with_world_async(world, |base_url| async move {
        let mut request = Client::default().request(method, format!("{base_url}{path}"));
        if let Some(cookie) = cookie {
            request = request.insert_header((header::COOKIE, cookie));
        }
        let mut response = match payload {
            Some(payload) => request.send_json(&payload).await.expect(label),
            None => request.send().await.expect(label),
        };
        let status = response.status().as_u16();
        let trace_id = response
            .headers()
            .get(TRACE_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_owned());
        let body = response.body().await.expect(label);
        let json: Value = serde_json::from_slice(&body).expect(label);
        (status, trace_id, json)
    });

    record_response(world, status, trace_id, body);
}

fn perform_get_current_user(world: &SharedWorld, include_cookie: bool) {
    perform_json_request(
        world,
        include_cookie,
        RequestSpec {
            method: Method::GET,
            path: "/api/v1/users/me",
            payload: None,
            label: "current user request",
        },
    );
}

fn perform_update_interests_payload(
    world: &SharedWorld,
    include_cookie: bool,
    payload: Value,
    label: &str,
) {
    perform_json_request(
        world,
        include_cookie,
        RequestSpec {
            method: Method::PUT,
            path: "/api/v1/users/me/interests",
            payload: Some(payload),
            label,
        },
    );
}

fn perform_update_interests(world: &SharedWorld, include_cookie: bool) {
    let payload = serde_json::json!({
        "interestThemeIds": ["3fa85f64-5717-4562-b3fc-2c963f66afa6"]
    });
    perform_update_interests_payload(world, include_cookie, payload, "update interests request");
}

fn perform_update_interests_with_invalid_id(world: &SharedWorld, include_cookie: bool) {
    let payload = serde_json::json!({
        "interestThemeIds": [""]
    });
    perform_update_interests_payload(
        world,
        include_cookie,
        payload,
        "update interests invalid request",
    );
}

#[given("a running server with session middleware")]
fn a_running_server_with_session_middleware(world: &WorldFixture) {
    let _ = world;
}

#[given("the client has an authenticated session")]
fn the_client_has_an_authenticated_session(world: &WorldFixture) {
    login_and_store_cookie(&world.world());
}

#[when("the client requests the current user without a session")]
fn the_client_requests_the_current_user_without_a_session(world: &WorldFixture) {
    perform_get_current_user(&world.world(), false);
}

#[when("the client updates interests without a session")]
fn the_client_updates_interests_without_a_session(world: &WorldFixture) {
    perform_update_interests(&world.world(), false);
}

#[when("the client requests the current user profile")]
fn the_client_requests_the_current_user_profile(world: &WorldFixture) {
    perform_get_current_user(&world.world(), true);
}

#[when("the client updates interest selections")]
fn the_client_updates_interest_selections(world: &WorldFixture) {
    perform_update_interests(&world.world(), true);
}

#[when("the client updates interests with an invalid interest theme id")]
fn the_client_updates_interests_with_an_invalid_interest_theme_id(world: &WorldFixture) {
    perform_update_interests_with_invalid_id(&world.world(), true);
}

#[then("the response is unauthorised with a trace id")]
fn the_response_is_unauthorised_with_a_trace_id(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    assert_eq!(ctx.last_status, Some(401));

    let trace_id = ctx.last_trace_id.as_deref().expect("trace id header");
    let body = ctx.last_body.as_ref().expect("error body");
    assert_eq!(body.get("traceId").and_then(Value::as_str), Some(trace_id));
}

#[then("the profile response includes the expected display name")]
fn the_profile_response_includes_the_expected_display_name(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    assert_eq!(ctx.last_status, Some(200));
    let body = ctx.last_body.as_ref().expect("profile body");
    assert_eq!(
        body.get("displayName").and_then(Value::as_str),
        Some("Ada Lovelace")
    );
}

#[then("the profile port was called with the authenticated user id")]
fn the_profile_port_was_called_with_the_authenticated_user_id(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    assert_eq!(
        ctx.profile.calls(),
        vec!["11111111-1111-1111-1111-111111111111".to_owned()]
    );
}

#[then("the interests response includes the selected theme")]
fn the_interests_response_includes_the_selected_theme(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    assert_eq!(ctx.last_status, Some(200));
    let body = ctx.last_body.as_ref().expect("interests body");
    let ids = body
        .get("interestThemeIds")
        .and_then(Value::as_array)
        .expect("interestThemeIds array");
    assert_eq!(
        ids.first().and_then(Value::as_str),
        Some("3fa85f64-5717-4562-b3fc-2c963f66afa6")
    );
}

#[then("the interests port was called with the authenticated user id and theme")]
fn the_interests_port_was_called_with_the_authenticated_user_id_and_theme(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    assert_eq!(
        ctx.interests.calls(),
        vec![(
            "11111111-1111-1111-1111-111111111111".to_owned(),
            vec!["3fa85f64-5717-4562-b3fc-2c963f66afa6".to_owned()],
        )]
    );
}

#[then("the response is a bad request with interest theme validation details")]
fn the_response_is_a_bad_request_with_interest_theme_validation_details(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    assert_eq!(ctx.last_status, Some(400));

    let body = ctx.last_body.as_ref().expect("error body");
    assert_eq!(
        body.get("message").and_then(Value::as_str),
        Some("interest theme id must not be empty")
    );
    assert_eq!(
        body.get("code").and_then(Value::as_str),
        Some("invalid_request")
    );

    let details = body
        .get("details")
        .and_then(Value::as_object)
        .expect("details object");
    assert_eq!(
        details.get("field").and_then(Value::as_str),
        Some("interestThemeIds")
    );
    assert_eq!(details.get("index").and_then(Value::as_i64), Some(0));
    assert_eq!(details.get("value").and_then(Value::as_str), Some(""));
    assert_eq!(
        details.get("code").and_then(Value::as_str),
        Some("empty_interest_theme_id")
    );
}
#[scenario(path = "tests/features/user_session.feature")]
fn user_session_scenarios(world: WorldFixture) {
    drop(world);
}
