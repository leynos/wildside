//! Behavioural tests for PWA preferences endpoints.
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

use actix_web::http::Method;
use backend::domain::ports::UpdatePreferencesResponse;
use backend::domain::{UnitSystem, UserId, UserPreferences};
use doubles::{UserPreferencesCommandResponse, UserPreferencesQueryResponse};
use harness::WorldFixture;
use pwa_http::{JsonRequest, login_and_store_cookie, perform_json_request};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use serde_json::Value;
use uuid::Uuid;

const AUTH_USER_ID: &str = "11111111-1111-1111-1111-111111111111";
const INTEREST_THEME_ID: &str = "3fa85f64-5717-4562-b3fc-2c963f66afa6";
const SAFETY_TOGGLE_ID: &str = "7fa85f64-5717-4562-b3fc-2c963f66afa6";
const IDEMPOTENCY_KEY: &str = "550e8400-e29b-41d4-a716-446655440000";

#[fixture]
fn world() -> WorldFixture {
    harness::world()
}

#[given("a running server with session middleware")]
fn a_running_server_with_session_middleware(world: &WorldFixture) {
    let _ = world;
}

#[given("the client has an authenticated session")]
fn the_client_has_an_authenticated_session(world: &WorldFixture) {
    let shared_world = world.world();
    login_and_store_cookie(&shared_world);
}

#[given("the preferences query returns default preferences")]
fn the_preferences_query_returns_default_preferences(world: &WorldFixture) {
    let world = world.world();
    let user_id = UserId::new(AUTH_USER_ID).expect("user id");
    let preferences = UserPreferences::builder(user_id)
        .interest_theme_ids(vec![Uuid::parse_str(INTEREST_THEME_ID).expect("theme id")])
        .safety_toggle_ids(vec![Uuid::parse_str(SAFETY_TOGGLE_ID).expect("safety id")])
        .unit_system(UnitSystem::Metric)
        .revision(1)
        .build();

    world
        .borrow()
        .preferences_query
        .set_response(UserPreferencesQueryResponse::Ok(preferences));
}

#[given("the preferences command returns updated preferences")]
fn the_preferences_command_returns_updated_preferences(world: &WorldFixture) {
    let world = world.world();
    let user_id = UserId::new(AUTH_USER_ID).expect("user id");
    let preferences = UserPreferences::builder(user_id)
        .interest_theme_ids(vec![Uuid::parse_str(INTEREST_THEME_ID).expect("theme id")])
        .safety_toggle_ids(vec![Uuid::parse_str(SAFETY_TOGGLE_ID).expect("safety id")])
        .unit_system(UnitSystem::Metric)
        .revision(2)
        .build();

    world
        .borrow()
        .preferences
        .set_response(UserPreferencesCommandResponse::Ok(
            UpdatePreferencesResponse {
                preferences,
                replayed: false,
            },
        ));
}

#[when("the client requests preferences")]
fn the_client_requests_preferences(world: &WorldFixture) {
    let shared_world = world.world();
    perform_json_request(
        &shared_world,
        JsonRequest {
            include_cookie: true,
            method: Method::GET,
            path: "/api/v1/users/me/preferences",
            payload: None,
            idempotency_key: None,
        },
    );
}

#[when("the client updates preferences with an invalid unit system")]
fn the_client_updates_preferences_with_an_invalid_unit_system(world: &WorldFixture) {
    let shared_world = world.world();
    perform_json_request(
        &shared_world,
        JsonRequest {
            include_cookie: true,
            method: Method::PUT,
            path: "/api/v1/users/me/preferences",
            payload: Some(serde_json::json!({
                "interestThemeIds": [INTEREST_THEME_ID],
                "safetyToggleIds": [SAFETY_TOGGLE_ID],
                "unitSystem": "unknown"
            })),
            idempotency_key: None,
        },
    );
}

#[when("the client updates preferences with an idempotency key")]
fn the_client_updates_preferences_with_an_idempotency_key(world: &WorldFixture) {
    let shared_world = world.world();
    perform_json_request(
        &shared_world,
        JsonRequest {
            include_cookie: true,
            method: Method::PUT,
            path: "/api/v1/users/me/preferences",
            payload: Some(serde_json::json!({
                "interestThemeIds": [INTEREST_THEME_ID],
                "safetyToggleIds": [SAFETY_TOGGLE_ID],
                "unitSystem": "metric",
                "expectedRevision": 1
            })),
            idempotency_key: Some(IDEMPOTENCY_KEY),
        },
    );
}

#[then("the response is ok")]
fn the_response_is_ok(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    assert_eq!(ctx.last_status, Some(200));
}

#[then("the preferences response includes the expected unit system")]
fn the_preferences_response_includes_the_expected_unit_system(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    let body = ctx.last_body.as_ref().expect("response body");
    assert_eq!(
        body.get("unitSystem").and_then(Value::as_str),
        Some("metric")
    );
}

#[then("the preferences response includes revision 2")]
fn the_preferences_response_includes_revision_2(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    let body = ctx.last_body.as_ref().expect("response body");
    assert_eq!(body.get("revision").and_then(Value::as_u64), Some(2));
}

#[then("the preferences query was called with the authenticated user id")]
fn the_preferences_query_was_called_with_the_authenticated_user_id(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    assert_eq!(ctx.preferences_query.calls(), vec![AUTH_USER_ID.to_owned()]);
}

#[then("the preferences command captures the idempotency key")]
fn the_preferences_command_captures_the_idempotency_key(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    let calls = ctx.preferences.calls();
    let request = calls.first().expect("preferences call");
    let idempotency_key = request.idempotency_key.as_ref().expect("idempotency key");
    assert_eq!(idempotency_key.to_string(), IDEMPOTENCY_KEY);
}

#[then("the response is a bad request with unit system details")]
fn the_response_is_a_bad_request_with_unit_system_details(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    assert_eq!(ctx.last_status, Some(400));
    let body = ctx.last_body.as_ref().expect("response body");
    assert_eq!(
        body.get("code").and_then(Value::as_str),
        Some("invalid_request")
    );
    let details = body
        .get("details")
        .and_then(Value::as_object)
        .expect("details");
    assert_eq!(
        details.get("field").and_then(Value::as_str),
        Some("unitSystem")
    );
}

#[scenario(path = "tests/features/pwa_preferences.feature")]
fn pwa_preferences(world: WorldFixture) {
    let _ = world;
}
