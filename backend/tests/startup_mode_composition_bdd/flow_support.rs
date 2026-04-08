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
use backend::inbound::http::users::{LoginRequest, current_user, login};
use backend::inbound::http::walk_sessions::create_walk_session;
use backend::outbound::persistence::{DbPool, PoolConfig};
use diesel_async::RunQueryDsl;
use pg_embedded_setup_unpriv::TemporaryDatabase;
use serde_json::Value;
use uuid::Uuid;

use super::support::atexit_cleanup::shared_cluster_handle;
use super::support::profile_interests::{FIXTURE_AUTH_ID, build_session_middleware};
use super::support::provision_template_database;
use super::{ServerConfig, state_builders};

/// Snapshot of an HTTP response for assertion purposes.
#[derive(Debug)]
pub(crate) struct Snapshot {
    pub(crate) status: u16,
    pub(crate) body: Option<Value>,
    pub(crate) trace_id: Option<String>,
    pub(crate) session_cookie: Option<Cookie<'static>>,
}

/// Database context for DB-present startup mode tests.
pub(crate) struct DbContext {
    pub(crate) database_url: String,
    pub(crate) pool: DbPool,
    _database: TemporaryDatabase,
}

/// BDD world state tracking all endpoint responses and startup mode.
pub(crate) struct World {
    pub(crate) db: Option<DbContext>,
    pub(crate) login: Option<Snapshot>,
    pub(crate) profile: Option<Snapshot>,
    #[allow(dead_code)]
    pub(crate) interests: Option<Snapshot>,
    pub(crate) preferences: Option<Snapshot>,
    pub(crate) catalogue_explore: Option<Snapshot>,
    pub(crate) catalogue_descriptors: Option<Snapshot>,
    pub(crate) offline_bundles: Option<Snapshot>,
    #[allow(dead_code)]
    pub(crate) walk_sessions: Option<Snapshot>,
    pub(crate) enrichment_provenance: Option<Snapshot>,
    pub(crate) skip_reason: Option<String>,
}

fn run_async<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Runtime::new()
        .expect("create runtime")
        .block_on(future)
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

/// Set up a DB context with embedded PostgreSQL.
pub(crate) fn setup_db_context() -> Result<DbContext, String> {
    let cluster = shared_cluster_handle().map_err(|error| error.to_string())?;
    let database = provision_template_database(cluster).map_err(|error| error.to_string())?;
    let database_url = database.url().to_owned();
    let pool = run_async(DbPool::new(
        PoolConfig::new(database_url.as_str())
            .with_max_size(2)
            .with_min_idle(Some(1)),
    ))
    .map_err(|error| error.to_string())?;
    Ok(DbContext {
        database_url,
        pool,
        _database: database,
    })
}

/// Seed a user into the DB for testing.
pub(crate) fn seed_user(pool: &DbPool, user_id: Uuid, display_name: &str) -> Result<(), String> {
    run_async(async {
        let mut conn = pool.get().await.map_err(|error| error.to_string())?;
        diesel::sql_query("INSERT INTO users (id, display_name) VALUES ($1, $2)")
            .bind::<diesel::sql_types::Uuid, _>(user_id)
            .bind::<diesel::sql_types::Text, _>(display_name)
            .execute(&mut conn)
            .await
            .map_err(|error| error.to_string())
            .map(|_| ())
    })
}

/// Execute the comprehensive startup-mode flow exercising all major port
/// groups.
pub(crate) fn run_comprehensive_flow(world: &mut World) {
    run_async(run_comprehensive_flow_async(world));
}

async fn run_comprehensive_flow_async(world: &mut World) {
    if is_skipped(world) {
        return;
    }

    let mut config = ServerConfig::new(
        Key::generate(),
        false, // cookie_secure
        SameSite::Lax,
        SocketAddr::from(([127, 0, 0, 1], 0)),
    );
    if let Some(ref db) = world.db {
        config = config.with_db_pool(db.pool.clone());
    }

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
    let login_payload = LoginRequest {
        username: "admin".to_owned(),
        password: "password".to_owned(),
    };
    let login_req = actix_test::TestRequest::post()
        .uri("/api/v1/login")
        .set_json(&login_payload)
        .to_request();

    let login_resp = actix_test::call_service(&app, login_req).await;
    let login_status = login_resp.status().as_u16();
    let login_trace_id = login_resp
        .headers()
        .get(TRACE_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(|s| s.to_owned());
    let session_cookie = login_resp
        .response()
        .cookies()
        .find(|cookie| cookie.name() == "session")
        .map(|cookie| cookie.into_owned());
    let login_body_bytes = actix_test::read_body(login_resp).await;
    let login_body = parse_json_body(&login_body_bytes);

    world.login = Some(Snapshot {
        status: login_status,
        body: login_body,
        trace_id: login_trace_id,
        session_cookie: session_cookie.clone(),
    });

    let cookie = match session_cookie.as_ref() {
        Some(c) => c,
        None => return, // Login failed, can't continue
    };

    // Profile GET
    let profile_req = actix_test::TestRequest::get()
        .uri("/api/v1/users/me")
        .cookie(cookie.clone())
        .to_request();
    let profile_resp = actix_test::call_service(&app, profile_req).await;
    let profile_status = profile_resp.status().as_u16();
    let profile_headers = profile_resp.headers().clone();
    let profile_body_bytes = actix_test::read_body(profile_resp).await;
    let profile_body = parse_json_body(&profile_body_bytes);
    let profile_trace_id = profile_headers
        .get(TRACE_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(|s| s.to_owned());
    world.profile = Some(Snapshot {
        status: profile_status,
        body: profile_body,
        trace_id: profile_trace_id,
        session_cookie: None,
    });

    // Preferences GET
    let prefs_get_req = actix_test::TestRequest::get()
        .uri("/api/v1/users/me/preferences")
        .cookie(cookie.clone())
        .to_request();
    let prefs_resp = actix_test::call_service(&app, prefs_get_req).await;
    let prefs_status = prefs_resp.status().as_u16();
    let prefs_headers = prefs_resp.headers().clone();
    let prefs_body_bytes = actix_test::read_body(prefs_resp).await;
    let prefs_body = parse_json_body(&prefs_body_bytes);
    let prefs_trace_id = prefs_headers
        .get(TRACE_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(|s| s.to_owned());
    world.preferences = Some(Snapshot {
        status: prefs_status,
        body: prefs_body,
        trace_id: prefs_trace_id,
        session_cookie: None,
    });

    // Catalogue explore GET
    let catalogue_explore_req = actix_test::TestRequest::get()
        .uri("/api/v1/catalogue/explore")
        .cookie(cookie.clone())
        .to_request();
    let catalogue_explore_resp = actix_test::call_service(&app, catalogue_explore_req).await;
    let catalogue_explore_status = catalogue_explore_resp.status().as_u16();
    let catalogue_explore_headers = catalogue_explore_resp.headers().clone();
    let catalogue_explore_body_bytes = actix_test::read_body(catalogue_explore_resp).await;
    let catalogue_explore_body = parse_json_body(&catalogue_explore_body_bytes);
    let catalogue_explore_trace_id = catalogue_explore_headers
        .get(TRACE_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(|s| s.to_owned());
    world.catalogue_explore = Some(Snapshot {
        status: catalogue_explore_status,
        body: catalogue_explore_body,
        trace_id: catalogue_explore_trace_id,
        session_cookie: None,
    });

    // Catalogue descriptors GET
    let catalogue_descriptors_req = actix_test::TestRequest::get()
        .uri("/api/v1/catalogue/descriptors")
        .cookie(cookie.clone())
        .to_request();
    let catalogue_descriptors_resp =
        actix_test::call_service(&app, catalogue_descriptors_req).await;
    let catalogue_descriptors_status = catalogue_descriptors_resp.status().as_u16();
    let catalogue_descriptors_headers = catalogue_descriptors_resp.headers().clone();
    let catalogue_descriptors_body_bytes = actix_test::read_body(catalogue_descriptors_resp).await;
    let catalogue_descriptors_body = parse_json_body(&catalogue_descriptors_body_bytes);
    let catalogue_descriptors_trace_id = catalogue_descriptors_headers
        .get(TRACE_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(|s| s.to_owned());
    world.catalogue_descriptors = Some(Snapshot {
        status: catalogue_descriptors_status,
        body: catalogue_descriptors_body,
        trace_id: catalogue_descriptors_trace_id,
        session_cookie: None,
    });

    // Offline bundles GET
    let offline_bundles_req = actix_test::TestRequest::get()
        .uri("/api/v1/offline/bundles?deviceId=test-device")
        .cookie(cookie.clone())
        .to_request();
    let offline_bundles_resp = actix_test::call_service(&app, offline_bundles_req).await;
    let offline_bundles_status = offline_bundles_resp.status().as_u16();
    let offline_bundles_headers = offline_bundles_resp.headers().clone();
    let offline_bundles_body_bytes = actix_test::read_body(offline_bundles_resp).await;
    let offline_bundles_body = parse_json_body(&offline_bundles_body_bytes);
    let offline_bundles_trace_id = offline_bundles_headers
        .get(TRACE_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(|s| s.to_owned());
    world.offline_bundles = Some(Snapshot {
        status: offline_bundles_status,
        body: offline_bundles_body,
        trace_id: offline_bundles_trace_id,
        session_cookie: None,
    });

    // Enrichment provenance GET (admin endpoint)
    let enrichment_req = actix_test::TestRequest::get()
        .uri("/api/v1/admin/enrichment/provenance")
        .cookie(cookie.clone())
        .to_request();
    let enrichment_resp = actix_test::call_service(&app, enrichment_req).await;
    let enrichment_status = enrichment_resp.status().as_u16();
    let enrichment_headers = enrichment_resp.headers().clone();
    let enrichment_body_bytes = actix_test::read_body(enrichment_resp).await;
    let enrichment_body = parse_json_body(&enrichment_body_bytes);
    let enrichment_trace_id = enrichment_headers
        .get(TRACE_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(|s| s.to_owned());
    world.enrichment_provenance = Some(Snapshot {
        status: enrichment_status,
        body: enrichment_body,
        trace_id: enrichment_trace_id,
        session_cookie: None,
    });
}

/// Execute flow with invalid input to test validation error stability.
pub(crate) fn run_validation_error_flow(world: &mut World) {
    run_async(run_validation_error_flow_async(world));
}

async fn run_validation_error_flow_async(world: &mut World) {
    if is_skipped(world) {
        return;
    }

    let mut config = ServerConfig::new(
        Key::generate(),
        false, // cookie_secure
        SameSite::Lax,
        SocketAddr::from(([127, 0, 0, 1], 0)),
    );
    if let Some(ref db) = world.db {
        config = config.with_db_pool(db.pool.clone());
    }

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
    let login_payload = LoginRequest {
        username: "admin".to_owned(),
        password: "password".to_owned(),
    };
    let login_req = actix_test::TestRequest::post()
        .uri("/api/v1/login")
        .set_json(&login_payload)
        .to_request();

    let login_resp = actix_test::call_service(&app, login_req).await;
    let session_cookie = login_resp
        .response()
        .cookies()
        .find(|c| c.name() == "session")
        .map(|c| c.into_owned());

    if session_cookie.is_none() {
        return; // Login failed
    }

    // Send invalid preferences request (missing required fields)
    let invalid_prefs = PreferencesRequest {
        interest_theme_ids: None, // Missing required field
        safety_toggle_ids: None,  // Missing required field
        unit_system: None,        // Missing required field
        expected_revision: None,
    };
    let prefs_req = actix_test::TestRequest::put()
        .uri("/api/v1/users/me/preferences")
        .cookie(session_cookie.expect("session cookie"))
        .insert_header(("idempotency-key", "550e8400-e29b-41d4-a716-446655440000"))
        .set_json(&invalid_prefs)
        .to_request();
    let prefs_resp = actix_test::call_service(&app, prefs_req).await;
    let prefs_status = prefs_resp.status().as_u16();
    let prefs_headers = prefs_resp.headers().clone();
    let prefs_body_bytes = actix_test::read_body(prefs_resp).await;
    let prefs_body = parse_json_body(&prefs_body_bytes);
    let prefs_trace_id = prefs_headers
        .get(TRACE_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(|s| s.to_owned());
    world.preferences = Some(Snapshot {
        status: prefs_status,
        body: prefs_body,
        trace_id: prefs_trace_id,
        session_cookie: None,
    });
}
