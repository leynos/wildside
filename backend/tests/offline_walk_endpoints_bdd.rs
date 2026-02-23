//! Behavioural tests for offline bundle and walk-session HTTP endpoints.
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
// Shared helpers include functions used only by other integration suites.
#[expect(
    dead_code,
    reason = "Shared helpers include functions used only by other integration suites."
)]
#[path = "support/bdd_common.rs"]
mod bdd_common;
#[expect(
    dead_code,
    reason = "Shared harness has extra fields used by other integration suites."
)]
#[path = "adapter_guardrails/harness.rs"]
mod harness;
#[path = "support/offline_walk_test_data.rs"]
mod offline_walk_test_data;
#[path = "support/pwa_http.rs"]
mod pwa_http;
#[path = "support/ws.rs"]
mod ws_support;

use actix_web::http::Method;
use backend::domain::ports::{
    CreateWalkSessionResponse, DeleteOfflineBundleResponse, ListOfflineBundlesResponse,
    UpsertOfflineBundleResponse,
};
use doubles::{
    DeleteOfflineBundleCommandResponse, OfflineBundleListQueryResponse,
    UpsertOfflineBundleCommandResponse, WalkSessionCommandResponse,
};
use harness::WorldFixture;
use offline_walk_test_data::{
    AUTH_USER_ID, BUNDLE_ID, IDEMPOTENCY_KEY, SESSION_ID, build_bundle_payload,
    build_walk_completion_summary, offline_upsert_payload_json, walk_session_payload_json,
};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use serde_json::Value;
use uuid::Uuid;
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
#[given("the offline bundle query returns one bundle")]
fn the_offline_bundle_query_returns_one_bundle(world: &WorldFixture) {
    world
        .world()
        .borrow()
        .offline_bundles_query
        .set_list_response(OfflineBundleListQueryResponse::Ok(
            ListOfflineBundlesResponse {
                bundles: vec![build_bundle_payload()],
            },
        ));
}
#[given("the offline bundle command returns an upserted bundle")]
fn the_offline_bundle_command_returns_an_upserted_bundle(world: &WorldFixture) {
    let bundle = build_bundle_payload();
    world.world().borrow().offline_bundles.set_upsert_response(
        UpsertOfflineBundleCommandResponse::Ok(UpsertOfflineBundleResponse {
            bundle,
            is_replayed: false,
        }),
    );
}
#[given("the offline bundle command returns a deleted bundle id")]
fn the_offline_bundle_command_returns_a_deleted_bundle_id(world: &WorldFixture) {
    world.world().borrow().offline_bundles.set_delete_response(
        DeleteOfflineBundleCommandResponse::Ok(DeleteOfflineBundleResponse {
            bundle_id: Uuid::parse_str(BUNDLE_ID).expect("bundle id"),
            is_replayed: false,
        }),
    );
}
#[given("the offline bundle command returns a replayed upsert bundle")]
fn the_offline_bundle_command_returns_a_replayed_upsert_bundle(world: &WorldFixture) {
    let bundle = build_bundle_payload();
    world.world().borrow().offline_bundles.set_upsert_response(
        UpsertOfflineBundleCommandResponse::Ok(UpsertOfflineBundleResponse {
            bundle,
            is_replayed: true,
        }),
    );
}
#[given("the offline bundle command returns a replayed deleted bundle id")]
fn the_offline_bundle_command_returns_a_replayed_deleted_bundle_id(world: &WorldFixture) {
    world.world().borrow().offline_bundles.set_delete_response(
        DeleteOfflineBundleCommandResponse::Ok(DeleteOfflineBundleResponse {
            bundle_id: Uuid::parse_str(BUNDLE_ID).expect("bundle id"),
            is_replayed: true,
        }),
    );
}
#[given("the walk session command returns a completion summary")]
fn the_walk_session_command_returns_a_completion_summary(world: &WorldFixture) {
    world
        .world()
        .borrow()
        .walk_sessions
        .set_response(WalkSessionCommandResponse::Ok(CreateWalkSessionResponse {
            session_id: Uuid::parse_str(SESSION_ID).expect("session id"),
            completion_summary: Some(build_walk_completion_summary()),
        }));
}
#[when("the client lists offline bundles for the ios device")]
fn the_client_lists_offline_bundles_for_the_ios_device(world: &WorldFixture) {
    bdd_common::perform_get_request(world, "/api/v1/offline/bundles?deviceId=ios-iphone-15");
}
#[when("the client upserts an offline bundle with idempotency key")]
fn the_client_upserts_an_offline_bundle_with_idempotency_key(world: &WorldFixture) {
    bdd_common::perform_mutation_request(
        world,
        bdd_common::MutationRequest {
            method: Method::POST,
            path: "/api/v1/offline/bundles",
            payload: offline_upsert_payload_json(),
            idempotency_key: Some(IDEMPOTENCY_KEY),
        },
    );
}
#[when("the client deletes an offline bundle with idempotency key")]
fn the_client_deletes_an_offline_bundle_with_idempotency_key(world: &WorldFixture) {
    bdd_common::perform_mutation_request(
        world,
        bdd_common::MutationRequest {
            method: Method::DELETE,
            path: "/api/v1/offline/bundles/00000000-0000-0000-0000-000000000101",
            payload: serde_json::json!({}),
            idempotency_key: Some(IDEMPOTENCY_KEY),
        },
    );
}
#[when("the client creates a walk session")]
fn the_client_creates_a_walk_session(world: &WorldFixture) {
    bdd_common::perform_mutation_request(
        world,
        bdd_common::MutationRequest {
            method: Method::POST,
            path: "/api/v1/walk-sessions",
            payload: walk_session_payload_json(),
            idempotency_key: None,
        },
    );
}
#[when("the client lists offline bundles without device id")]
fn the_client_lists_offline_bundles_without_device_id(world: &WorldFixture) {
    bdd_common::perform_get_request(world, "/api/v1/offline/bundles");
}
#[when("the client lists offline bundles with blank device id")]
fn the_client_lists_offline_bundles_with_blank_device_id(world: &WorldFixture) {
    bdd_common::perform_get_request(world, "/api/v1/offline/bundles?deviceId=%20%20");
}
#[when("the client upserts an offline bundle with invalid idempotency key")]
fn the_client_upserts_an_offline_bundle_with_invalid_idempotency_key(world: &WorldFixture) {
    let shared_world = world.world();
    pwa_http::perform_json_request(
        &shared_world,
        pwa_http::JsonRequest {
            include_cookie: true,
            method: Method::POST,
            path: "/api/v1/offline/bundles",
            payload: Some(offline_upsert_payload_json()),
            idempotency_key: Some("not-a-uuid"),
        },
    );
}
#[when("the unauthenticated client creates a walk session")]
fn the_unauthenticated_client_creates_a_walk_session(world: &WorldFixture) {
    let shared_world = world.world();
    pwa_http::perform_json_request(
        &shared_world,
        pwa_http::JsonRequest {
            include_cookie: false,
            method: Method::POST,
            path: "/api/v1/walk-sessions",
            payload: Some(walk_session_payload_json()),
            idempotency_key: None,
        },
    );
}
#[then("the response is ok")]
fn the_response_is_ok(world: &WorldFixture) {
    bdd_common::assert_response_ok(world);
}
#[then("the offline list response includes the configured bundle id")]
fn the_offline_list_response_includes_the_configured_bundle_id(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    let body = ctx.last_body.as_ref().expect("response body");
    let bundles = body
        .get("bundles")
        .and_then(Value::as_array)
        .expect("bundles array");
    assert_eq!(
        bundles
            .first()
            .and_then(|bundle| bundle.get("id"))
            .and_then(Value::as_str),
        Some(BUNDLE_ID)
    );
}
#[then("the offline list query captures session user and ios device")]
fn the_offline_list_query_captures_session_user_and_ios_device(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    let calls = ctx.offline_bundles_query.list_calls();
    let request = calls.first().expect("offline list call");
    assert_eq!(
        request
            .owner_user_id
            .as_ref()
            .map(ToString::to_string)
            .as_deref(),
        Some(AUTH_USER_ID)
    );
    assert_eq!(request.device_id, "ios-iphone-15");
}
#[then("the offline upsert response includes the configured bundle id")]
fn the_offline_upsert_response_includes_the_configured_bundle_id(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    let body = ctx.last_body.as_ref().expect("response body");
    assert_eq!(
        body.get("bundleId").and_then(Value::as_str),
        Some(BUNDLE_ID)
    );
}
#[then("the offline upsert command captures the idempotency key")]
fn the_offline_upsert_command_captures_the_idempotency_key(world: &WorldFixture) {
    bdd_common::assert_idempotency_key_captured(
        world,
        |ctx| {
            let calls = ctx.offline_bundles.upsert_calls();
            let request = calls.first().expect("offline upsert call");
            request.idempotency_key.as_ref().map(ToString::to_string)
        },
        IDEMPOTENCY_KEY,
    );
}
#[then("the offline delete response includes the configured bundle id")]
fn the_offline_delete_response_includes_the_configured_bundle_id(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    let body = ctx.last_body.as_ref().expect("response body");
    assert_eq!(
        body.get("bundleId").and_then(Value::as_str),
        Some(BUNDLE_ID)
    );
}
#[then("the offline delete command captures the idempotency key")]
fn the_offline_delete_command_captures_the_idempotency_key(world: &WorldFixture) {
    bdd_common::assert_idempotency_key_captured(
        world,
        |ctx| {
            let calls = ctx.offline_bundles.delete_calls();
            let request = calls.first().expect("offline delete call");
            request.idempotency_key.as_ref().map(ToString::to_string)
        },
        IDEMPOTENCY_KEY,
    );
}
#[then("the walk session response includes the configured session id")]
fn the_walk_session_response_includes_the_configured_session_id(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    let body = ctx.last_body.as_ref().expect("response body");
    assert_eq!(
        body.get("sessionId").and_then(Value::as_str),
        Some(SESSION_ID)
    );
}
#[then("the walk session response includes completion summary")]
fn the_walk_session_response_includes_completion_summary(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    let body = ctx.last_body.as_ref().expect("response body");
    assert!(body.get("completionSummary").is_some());
}
#[then("the walk session command captures the session id")]
fn the_walk_session_command_captures_the_session_id(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    let calls = ctx.walk_sessions.calls();
    let request = calls.first().expect("walk session call");
    assert_eq!(
        request.session.id,
        Uuid::parse_str(SESSION_ID).expect("session id")
    );
}
#[then("the response is bad request")]
fn the_response_is_bad_request(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    assert_eq!(ctx.last_status, Some(400));
}
#[then("the response is bad request with device id validation details")]
fn the_response_is_bad_request_with_device_id_validation_details(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    assert_eq!(ctx.last_status, Some(400));
    let body = ctx.last_body.as_ref().expect("response body");
    let details = body
        .get("details")
        .and_then(Value::as_object)
        .expect("details");
    assert_eq!(
        details.get("field").and_then(Value::as_str),
        Some("deviceId")
    );
    assert_eq!(
        details.get("code").and_then(Value::as_str),
        Some("invalid_device_id")
    );
}
#[then("the response indicates replayed idempotent result")]
fn the_response_indicates_replayed_idempotent_result(world: &WorldFixture) {
    bdd_common::assert_response_replayed(world);
}
#[then("the response is unauthorized")]
fn the_response_is_unauthorized(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    assert_eq!(ctx.last_status, Some(401));
}
#[scenario(path = "tests/features/offline_walk_endpoints.feature")]
fn offline_walk_endpoints(world: WorldFixture) {
    drop(world);
}
