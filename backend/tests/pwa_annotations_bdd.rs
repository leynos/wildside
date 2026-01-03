//! Behavioural tests for PWA annotations endpoints.
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
use backend::domain::ports::UpsertNoteResponse;
use backend::domain::{Error, RouteAnnotations, RouteNote, RouteProgress, UserId};
use doubles::{
    RouteAnnotationsQueryResponse, UpdateProgressCommandResponse, UpsertNoteCommandResponse,
};
use harness::WorldFixture;
use pwa_http::{JsonRequest, login_and_store_cookie, perform_json_request};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use serde_json::Value;
use uuid::Uuid;

const AUTH_USER_ID: &str = "11111111-1111-1111-1111-111111111111";
const ROUTE_ID: &str = "33333333-3333-3333-3333-333333333333";
const NOTE_ID: &str = "44444444-4444-4444-4444-444444444444";
const POI_ID: &str = "55555555-5555-5555-5555-555555555555";
const STOP_ID: &str = "66666666-6666-6666-6666-666666666666";
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

#[given("the annotations query returns a note and progress")]
fn the_annotations_query_returns_a_note_and_progress(world: &WorldFixture) {
    let world = world.world();
    let user_id = UserId::new(AUTH_USER_ID).expect("user id");
    let route_id = Uuid::parse_str(ROUTE_ID).expect("route id");
    let note_id = Uuid::parse_str(NOTE_ID).expect("note id");
    let poi_id = Uuid::parse_str(POI_ID).expect("poi id");
    let stop_id = Uuid::parse_str(STOP_ID).expect("stop id");
    let note = RouteNote::builder(note_id, route_id, user_id.clone())
        .poi_id(poi_id)
        .body("First note")
        .revision(1)
        .build();
    let progress = RouteProgress::builder(route_id, user_id.clone())
        .visited_stop_ids(vec![stop_id])
        .revision(1)
        .build();

    world
        .borrow()
        .route_annotations_query
        .set_response(RouteAnnotationsQueryResponse::Ok(RouteAnnotations {
            route_id,
            notes: vec![note],
            progress: Some(progress),
        }));
}

#[given("the annotations command returns an upserted note")]
fn the_annotations_command_returns_an_upserted_note(world: &WorldFixture) {
    let world = world.world();
    let user_id = UserId::new(AUTH_USER_ID).expect("user id");
    let route_id = Uuid::parse_str(ROUTE_ID).expect("route id");
    let note_id = Uuid::parse_str(NOTE_ID).expect("note id");
    let poi_id = Uuid::parse_str(POI_ID).expect("poi id");
    let note = RouteNote::builder(note_id, route_id, user_id)
        .poi_id(poi_id)
        .body("Upserted note")
        .revision(1)
        .build();

    world
        .borrow()
        .route_annotations
        .set_upsert_response(UpsertNoteCommandResponse::Ok(UpsertNoteResponse {
            note,
            replayed: false,
        }));
}

#[given("the progress update is configured to conflict")]
fn the_progress_update_is_configured_to_conflict(world: &WorldFixture) {
    let world = world.world();
    world
        .borrow()
        .route_annotations
        .set_update_response(UpdateProgressCommandResponse::Err(Error::conflict(
            "revision mismatch",
        )));
}

#[when("the client requests annotations for the route")]
fn the_client_requests_annotations_for_the_route(world: &WorldFixture) {
    let shared_world = world.world();
    perform_json_request(
        &shared_world,
        JsonRequest {
            include_cookie: true,
            method: Method::GET,
            path: &format!("/api/v1/routes/{}/annotations", ROUTE_ID),
            payload: None,
            idempotency_key: None,
        },
    );
}

#[when("the client upserts a note with an idempotency key")]
fn the_client_upserts_a_note_with_an_idempotency_key(world: &WorldFixture) {
    let shared_world = world.world();
    perform_json_request(
        &shared_world,
        JsonRequest {
            include_cookie: true,
            method: Method::POST,
            path: &format!("/api/v1/routes/{}/notes", ROUTE_ID),
            payload: Some(serde_json::json!({
                "noteId": NOTE_ID,
                "poiId": POI_ID,
                "body": "New note",
                "expectedRevision": null
            })),
            idempotency_key: Some(IDEMPOTENCY_KEY),
        },
    );
}

#[when("the client updates progress with a valid payload")]
fn the_client_updates_progress_with_a_valid_payload(world: &WorldFixture) {
    let shared_world = world.world();
    perform_json_request(
        &shared_world,
        JsonRequest {
            include_cookie: true,
            method: Method::PUT,
            path: &format!("/api/v1/routes/{}/progress", ROUTE_ID),
            payload: Some(serde_json::json!({
                "visitedStopIds": [STOP_ID],
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

#[then("the annotations response includes the note and progress")]
fn the_annotations_response_includes_the_note_and_progress(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    let body = ctx.last_body.as_ref().expect("response body");
    let notes = body
        .get("notes")
        .and_then(Value::as_array)
        .expect("notes array");
    let first_note = notes.first().expect("note");
    assert_eq!(first_note.get("id").and_then(Value::as_str), Some(NOTE_ID));
    let progress = body.get("progress").expect("progress");
    let visited = progress
        .get("visitedStopIds")
        .and_then(Value::as_array)
        .expect("visitedStopIds");
    assert_eq!(visited.first().and_then(Value::as_str), Some(STOP_ID));
}

#[then("the annotations query was called with the authenticated user id")]
fn the_annotations_query_was_called_with_the_authenticated_user_id(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    let calls = ctx.route_annotations_query.calls();
    let (route_id, user_id) = calls.first().expect("annotations call");
    assert_eq!(*route_id, Uuid::parse_str(ROUTE_ID).expect("route id"));
    assert_eq!(user_id, AUTH_USER_ID);
}

#[then("the note response includes the note id")]
fn the_note_response_includes_the_note_id(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    let body = ctx.last_body.as_ref().expect("response body");
    assert_eq!(body.get("id").and_then(Value::as_str), Some(NOTE_ID));
}

#[then("the note command captures the idempotency key")]
fn the_note_command_captures_the_idempotency_key(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    let calls = ctx.route_annotations.upsert_calls();
    let request = calls.first().expect("note upsert call");
    let idempotency_key = request.idempotency_key.as_ref().expect("idempotency key");
    assert_eq!(idempotency_key.to_string(), IDEMPOTENCY_KEY);
}

#[then("the response is a conflict error")]
fn the_response_is_a_conflict_error(world: &WorldFixture) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    assert_eq!(ctx.last_status, Some(409));
    let body = ctx.last_body.as_ref().expect("response body");
    assert_eq!(body.get("code").and_then(Value::as_str), Some("conflict"));
}

#[scenario(path = "tests/features/pwa_annotations.feature")]
fn pwa_annotations(world: WorldFixture) {
    let _ = world;
}
