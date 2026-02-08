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

use actix_web::http::Method;
use backend::domain::ports::UpdatePreferencesResponse;
use backend::domain::{UnitSystem, UserId, UserPreferences};
use doubles::{UserPreferencesCommandResponse, UserPreferencesQueryResponse};
use harness::WorldFixture;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use serde_json::Value;
use uuid::Uuid;

const AUTH_USER_ID: &str = "11111111-1111-1111-1111-111111111111";
const INTEREST_THEME_ID: &str = "3fa85f64-5717-4562-b3fc-2c963f66afa6";
const SAFETY_TOGGLE_ID: &str = "7fa85f64-5717-4562-b3fc-2c963f66afa6";
const IDEMPOTENCY_KEY: &str = "550e8400-e29b-41d4-a716-446655440000";

fn build_preferences(user_id: UserId, revision: u32) -> UserPreferences {
    UserPreferences::builder(user_id)
        .interest_theme_ids(vec![Uuid::parse_str(INTEREST_THEME_ID).expect("theme id")])
        .safety_toggle_ids(vec![Uuid::parse_str(SAFETY_TOGGLE_ID).expect("safety id")])
        .unit_system(UnitSystem::Metric)
        .revision(revision)
        .build()
}

fn preferences_payload(unit_system: &str, expected_revision: Option<u32>) -> Value {
    let mut payload = serde_json::json!({
        "interestThemeIds": [INTEREST_THEME_ID],
        "safetyToggleIds": [SAFETY_TOGGLE_ID],
        "unitSystem": unit_system
    });

    if let Some(expected) = expected_revision {
        let object = payload.as_object_mut().expect("preferences payload object");
        object.insert("expectedRevision".to_owned(), serde_json::json!(expected));
    }

    payload
}

fn perform_preferences_update(world: &WorldFixture, payload: Value, idempotency_key: Option<&str>) {
    bdd_common::perform_mutation_request(
        world,
        bdd_common::MutationRequest {
            method: Method::PUT,
            path: "/api/v1/users/me/preferences",
            payload,
            idempotency_key,
        },
    );
}

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

#[given("the preferences query returns default preferences")]
fn the_preferences_query_returns_default_preferences(world: &WorldFixture) {
    let world = world.world();
    let user_id = UserId::new(AUTH_USER_ID).expect("user id");
    let preferences = build_preferences(user_id, 1);

    world
        .borrow()
        .preferences_query
        .set_response(UserPreferencesQueryResponse::Ok(preferences));
}

#[given("the preferences command returns updated preferences")]
fn the_preferences_command_returns_updated_preferences(world: &WorldFixture) {
    let world = world.world();
    let user_id = UserId::new(AUTH_USER_ID).expect("user id");
    let preferences = build_preferences(user_id, 2);

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
    bdd_common::perform_get_request(world, "/api/v1/users/me/preferences");
}

#[when("the client updates preferences with an invalid unit system")]
fn the_client_updates_preferences_with_an_invalid_unit_system(world: &WorldFixture) {
    perform_preferences_update(world, preferences_payload("unknown", None), None);
}

#[when("the client updates preferences with an idempotency key")]
fn the_client_updates_preferences_with_an_idempotency_key(world: &WorldFixture) {
    perform_preferences_update(
        world,
        preferences_payload("metric", Some(1)),
        Some(IDEMPOTENCY_KEY),
    );
}

#[then("the response is ok")]
fn the_response_is_ok(world: &WorldFixture) {
    bdd_common::assert_response_ok(world);
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
    bdd_common::assert_idempotency_key_captured(
        world,
        |ctx| {
            let calls = ctx.preferences.calls();
            let request = calls.first().expect("preferences call");
            request
                .idempotency_key
                .as_ref()
                .map(|key: &backend::domain::IdempotencyKey| key.to_string())
        },
        IDEMPOTENCY_KEY,
    );
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

#[given("the preferences command returns a revision mismatch")]
fn the_preferences_command_returns_a_revision_mismatch(world: &WorldFixture) {
    world
        .world()
        .borrow()
        .preferences
        .set_response(UserPreferencesCommandResponse::Err(
            bdd_common::revision_mismatch_error(1, 2),
        ));
}

#[given("the preferences command returns an idempotency conflict")]
fn the_preferences_command_returns_an_idempotency_conflict(world: &WorldFixture) {
    world
        .world()
        .borrow()
        .preferences
        .set_response(UserPreferencesCommandResponse::Err(
            bdd_common::idempotency_conflict_error(),
        ));
}

#[given("the preferences command returns a replayed response")]
fn the_preferences_command_returns_a_replayed_response(world: &WorldFixture) {
    let world = world.world();
    let user_id = UserId::new(AUTH_USER_ID).expect("user id");
    let preferences = build_preferences(user_id, 2);

    world
        .borrow()
        .preferences
        .set_response(UserPreferencesCommandResponse::Ok(
            UpdatePreferencesResponse {
                preferences,
                replayed: true,
            },
        ));
}

#[when("the client updates preferences with expected revision 1")]
fn the_client_updates_preferences_with_expected_revision_1(world: &WorldFixture) {
    perform_preferences_update(world, preferences_payload("metric", Some(1)), None);
}

common_conflict_response_steps!();
replayed_response_step!(
    the_preferences_response_includes_replayed_true,
    "the preferences response includes replayed true"
);

#[scenario(path = "tests/features/pwa_preferences.feature")]
fn pwa_preferences(world: WorldFixture) {
    drop(world);
}
