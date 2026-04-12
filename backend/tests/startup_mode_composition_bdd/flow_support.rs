//! Shared flow and assertion helpers for comprehensive startup-mode
//! composition BDD.
//!
//! This module provides test utilities for exercising all 16 HTTP-facing ports
//! across both fixture-fallback and DB-present startup modes, proving that
//! adapter selection remains deterministic as wiring evolves.

use std::net::SocketAddr;
use std::sync::Arc;

use actix_web::cookie::{Cookie, Key, SameSite};
use actix_web::{App, test as actix_test, web};
use backend::domain::TRACE_ID_HEADER;
use backend::domain::ports::{FixtureRouteSubmissionService, RouteSubmissionService};
use backend::inbound::http::admin_enrichment::list_enrichment_provenance;
use backend::inbound::http::catalogue::{get_descriptors, get_explore_catalogue};
use backend::inbound::http::offline::list_offline_bundles;
use backend::inbound::http::preferences::{
    PreferencesRequest, get_preferences, update_preferences,
};
use backend::inbound::http::users::{LoginRequest, current_user, login, update_interests};
use backend::inbound::http::walk_sessions::create_walk_session;
use serde_json::Value;
use uuid::Uuid;

use super::db_support::DbContext;
use super::support::profile_interests::{FIXTURE_AUTH_ID, build_session_middleware};
use super::{ServerConfig, state_builders};

/// Snapshot of an HTTP response for assertion purposes.
#[derive(Debug)]
pub(crate) struct Snapshot {
    pub(crate) status: u16,
    pub(crate) body: Option<Value>,
    pub(crate) trace_id: Option<String>,
    pub(crate) session_cookie: Option<Cookie<'static>>,
}

/// BDD world state tracking all endpoint responses and startup mode.
pub(crate) struct World {
    pub(crate) runtime: Arc<tokio::runtime::Runtime>,
    pub(crate) db: Option<DbContext>,
    pub(crate) seeded_route_id: Option<Uuid>,
    pub(crate) login: Option<Snapshot>,
    pub(crate) profile: Option<Snapshot>,
    pub(crate) interests: Option<Snapshot>,
    pub(crate) preferences: Option<Snapshot>,
    pub(crate) catalogue_explore: Option<Snapshot>,
    pub(crate) catalogue_descriptors: Option<Snapshot>,
    pub(crate) offline_bundles: Option<Snapshot>,
    pub(crate) walk_sessions: Option<Snapshot>,
    pub(crate) enrichment_provenance: Option<Snapshot>,
    pub(crate) skip_reason: Option<String>,
}

fn parse_json_body(bytes: &[u8]) -> Option<Value> {
    (!bytes.is_empty()).then(|| serde_json::from_slice(bytes).expect("json body"))
}

/// Check if the scenario should be skipped due to cluster setup failure.
pub(crate) fn is_skipped(world: &World) -> bool {
    if let Some(reason) = world.skip_reason.as_deref() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped ({reason})");
        true
    } else {
        false
    }
}

/// Assert that a snapshot represents a 500 Internal Server Error with stable
/// error envelope.
pub(crate) fn assert_internal(snapshot: &Snapshot) {
    assert_eq!(snapshot.status, 500);
    let body = snapshot.body.as_ref().expect("error body");
    assert_eq!(
        body.get("message").and_then(Value::as_str),
        Some("Internal server error")
    );
    assert_eq!(
        body.get("code").and_then(Value::as_str),
        Some("internal_error")
    );
    let trace_header = snapshot.trace_id.as_deref().expect("trace-id header");
    let trace_body = body
        .get("traceId")
        .and_then(Value::as_str)
        .expect("traceId body");
    assert_eq!(trace_header, trace_body);
}

/// Assert that a profile response matches the expected display name.
pub(crate) fn assert_profile_response(snapshot: &Snapshot, expected_display_name: &str) {
    assert_eq!(snapshot.status, 200);
    let body = snapshot.body.as_ref().expect("profile body");
    assert_eq!(
        body.get("id").and_then(Value::as_str),
        Some(FIXTURE_AUTH_ID)
    );
    assert_eq!(
        body.get("displayName").and_then(Value::as_str),
        Some(expected_display_name)
    );
}

/// Build a server configuration from the world state.
fn build_server_config(world: &World) -> ServerConfig {
    let mut config = ServerConfig::new(
        Key::generate(),
        false, // cookie_secure
        SameSite::Lax,
        SocketAddr::from(([127, 0, 0, 1], 0)),
    );
    if let Some(ref db) = world.db {
        config = config.with_db_pool(db.pool.clone());
    }
    config
}

/// Capture a snapshot from a service response.
async fn capture_snapshot<B>(
    resp: actix_web::dev::ServiceResponse<B>,
    extract_cookie: bool,
) -> Snapshot
where
    B: actix_web::body::MessageBody,
{
    let status = resp.status().as_u16();
    let trace_id = resp
        .headers()
        .get(TRACE_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(|s| s.to_owned());
    let session_cookie = if extract_cookie {
        resp.response()
            .cookies()
            .find(|cookie| cookie.name() == "session")
            .map(|cookie| cookie.into_owned())
    } else {
        None
    };
    let body_bytes = actix_test::read_body(resp).await;
    let body = parse_json_body(&body_bytes);
    Snapshot {
        status,
        body,
        trace_id,
        session_cookie,
    }
}

/// Perform login and return a snapshot.
async fn perform_login(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
) -> Snapshot {
    let login_payload = LoginRequest {
        username: "admin".to_owned(),
        password: "password".to_owned(),
    };
    let login_req = actix_test::TestRequest::post()
        .uri("/api/v1/login")
        .set_json(&login_payload)
        .to_request();
    let login_resp = actix_test::call_service(app, login_req).await;
    capture_snapshot(login_resp, true).await
}

/// Call an authenticated GET endpoint and return a snapshot.
async fn call_get(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    uri: &str,
    cookie: Cookie<'static>,
) -> Snapshot {
    let req = actix_test::TestRequest::get()
        .uri(uri)
        .cookie(cookie)
        .to_request();
    let resp = actix_test::call_service(app, req).await;
    capture_snapshot(resp, false).await
}

/// Call an authenticated JSON-body endpoint and return a snapshot.
async fn call_json(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    req: actix_test::TestRequest,
    cookie: Cookie<'static>,
) -> Snapshot {
    let resp = actix_test::call_service(app, req.cookie(cookie).to_request()).await;
    capture_snapshot(resp, false).await
}

/// Execute the comprehensive startup-mode flow exercising all major port
/// groups.
pub(crate) fn run_comprehensive_flow(world: &mut World) {
    let rt = Arc::clone(&world.runtime);
    rt.block_on(run_comprehensive_flow_async(world));
}

async fn run_comprehensive_flow_async(world: &mut World) {
    if is_skipped(world) {
        return;
    }

    let config = build_server_config(world);
    let route_submission: Arc<dyn RouteSubmissionService> = Arc::new(FixtureRouteSubmissionService);
    let state = state_builders::build_http_state(&config, route_submission);

    let app = actix_test::init_service(
        App::new()
            .app_data(state.clone())
            .wrap(backend::Trace)
            .service(
                web::scope("/api/v1")
                    .wrap(build_session_middleware())
                    .service(login)
                    .service(current_user)
                    .service(update_interests)
                    .service(get_preferences)
                    .service(update_preferences)
                    .service(get_explore_catalogue)
                    .service(get_descriptors)
                    .service(list_offline_bundles)
                    .service(create_walk_session)
                    .service(list_enrichment_provenance),
            ),
    )
    .await;

    // Login to establish session
    let login_snapshot = perform_login(&app).await;
    let cookie = match login_snapshot.session_cookie.as_ref() {
        Some(c) => c.clone(),
        None => {
            world.login = Some(login_snapshot);
            return; // Login failed, can't continue
        }
    };
    world.login = Some(login_snapshot);

    // Exercise all major port groups (GET endpoints)
    world.profile = Some(call_get(&app, "/api/v1/users/me", cookie.clone()).await);
    world.preferences = Some(call_get(&app, "/api/v1/users/me/preferences", cookie.clone()).await);
    world.catalogue_explore =
        Some(call_get(&app, "/api/v1/catalogue/explore", cookie.clone()).await);
    world.catalogue_descriptors =
        Some(call_get(&app, "/api/v1/catalogue/descriptors", cookie.clone()).await);
    world.offline_bundles = Some(
        call_get(
            &app,
            "/api/v1/offline/bundles?deviceId=test-device",
            cookie.clone(),
        )
        .await,
    );
    world.enrichment_provenance =
        Some(call_get(&app, "/api/v1/admin/enrichment/provenance", cookie.clone()).await);

    // Exercise interests port (PUT with a minimal valid payload).
    // The preceding GET preferences call may have created default preferences
    // (revision 1), so we forward that revision to satisfy optimistic-lock
    // validation in DB-present mode.
    let expected_revision = world
        .preferences
        .as_ref()
        .and_then(|s| s.body.as_ref())
        .and_then(|b| b.get("revision"))
        .and_then(|v| v.as_u64())
        .map(|r| r as u32);
    let interests_req = actix_test::TestRequest::put()
        .uri("/api/v1/users/me/interests")
        .set_json(serde_json::json!({
            "interestThemeIds": ["00000000-0000-0000-0000-000000000001"],
            "expectedRevision": expected_revision,
        }));
    world.interests = Some(call_json(&app, interests_req, cookie.clone()).await);

    // Exercise walk sessions port (POST with a minimal valid payload).
    // In DB-present mode the routes table enforces a foreign key, so use
    // the seeded route_id; in fixture mode any UUID is accepted.
    let route_id = world.seeded_route_id.unwrap_or_else(Uuid::new_v4);
    let walk_req = actix_test::TestRequest::post()
        .uri("/api/v1/walk-sessions")
        .set_json(serde_json::json!({
            "id": Uuid::new_v4().to_string(),
            "routeId": route_id.to_string(),
            "startedAt": "2026-01-01T00:00:00Z",
            "primaryStats": [],
            "secondaryStats": [],
            "highlightedPoiIds": [],
        }));
    world.walk_sessions = Some(call_json(&app, walk_req, cookie).await);
}

/// Execute flow with invalid input to test validation error stability.
pub(crate) fn run_validation_error_flow(world: &mut World) {
    let rt = Arc::clone(&world.runtime);
    rt.block_on(run_validation_error_flow_async(world));
}

async fn run_validation_error_flow_async(world: &mut World) {
    if is_skipped(world) {
        return;
    }

    let config = build_server_config(world);
    let route_submission: Arc<dyn RouteSubmissionService> = Arc::new(FixtureRouteSubmissionService);
    let state = state_builders::build_http_state(&config, route_submission);

    let app = actix_test::init_service(
        App::new()
            .app_data(state.clone())
            .wrap(backend::Trace)
            .service(
                web::scope("/api/v1")
                    .wrap(build_session_middleware())
                    .service(login)
                    .service(update_preferences),
            ),
    )
    .await;

    // Login to establish session
    let login_snapshot = perform_login(&app).await;
    let cookie = match login_snapshot.session_cookie {
        Some(c) => c,
        None => return, // Login failed
    };

    // Send invalid preferences request (missing required fields)
    let invalid_prefs = PreferencesRequest {
        interest_theme_ids: None, // Missing required field
        safety_toggle_ids: None,  // Missing required field
        unit_system: None,        // Missing required field
        expected_revision: None,
    };
    let prefs_req = actix_test::TestRequest::put()
        .uri("/api/v1/users/me/preferences")
        .cookie(cookie)
        .insert_header(("idempotency-key", "550e8400-e29b-41d4-a716-446655440000"))
        .set_json(&invalid_prefs)
        .to_request();
    let prefs_resp = actix_test::call_service(&app, prefs_req).await;
    world.preferences = Some(capture_snapshot(prefs_resp, false).await);
}
