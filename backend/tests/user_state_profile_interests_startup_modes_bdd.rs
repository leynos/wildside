//! Behaviour coverage for 3.5.3 profile/interests startup-mode stability.
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;

use actix_session::SessionMiddleware;
use actix_session::config::{CookieContentSecurity, PersistentSession};
use actix_session::storage::CookieSessionStore;
use actix_web::cookie::{Cookie, Key, SameSite, time::Duration as CookieDuration};
use actix_web::{App, test as actix_test, web};
use backend::domain::TRACE_ID_HEADER;
use backend::domain::ports::{FixtureRouteSubmissionService, RouteSubmissionService};
use backend::inbound::http::users::{
    InterestsRequest, LoginRequest, current_user, login, update_interests,
};
use backend::outbound::persistence::{DbPool, PoolConfig};
use pg_embedded_setup_unpriv::TemporaryDatabase;
use postgres::{Client, NoTls};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use serde_json::Value;
use uuid::Uuid;

mod support;

use support::atexit_cleanup::shared_cluster_handle;
use support::{
    drop_table, format_postgres_error, handle_cluster_setup_failure, provision_template_database,
};

#[path = "../src/server/config.rs"]
#[expect(
    dead_code,
    reason = "tests import ServerConfig from server_config for BDD startup-mode checks"
)]
mod server_config;
pub use server_config::ServerConfig;

#[path = "../src/server/state_builders.rs"]
mod state_builders;

const FIXTURE_AUTH_ID: &str = "123e4567-e89b-12d3-a456-426614174000";
const DB_PROFILE_NAME: &str = "Database Ada";
const FIRST_THEME_ID: &str = "3fa85f64-5717-4562-b3fc-2c963f66afa6";
const INTEREST_THEME_IDS_MAX: usize = 100;

#[derive(Debug)]
struct Snapshot {
    status: u16,
    body: Option<Value>,
    trace_id: Option<String>,
    session_cookie: Option<Cookie<'static>>,
}

struct DbContext {
    database_url: String,
    pool: DbPool,
    _database: TemporaryDatabase,
}

struct World {
    db: Option<DbContext>,
    login: Option<Snapshot>,
    profile: Option<Snapshot>,
    interests: Option<Snapshot>,
    interests_payload: InterestsRequest,
    skip_reason: Option<String>,
}

fn run_async<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(future)
}

fn parse_json_body(bytes: &[u8]) -> Option<Value> {
    (!bytes.is_empty()).then(|| serde_json::from_slice(bytes).expect("json body"))
}

fn assert_internal(snapshot: &Snapshot) {
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

fn is_skipped(world: &World) -> bool {
    if let Some(reason) = world.skip_reason.as_deref() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped ({reason})");
        true
    } else {
        false
    }
}

fn setup_db_context() -> Result<DbContext, String> {
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

fn seed_user(url: &str, user_id: &str, display_name: &str) -> Result<(), String> {
    let mut client = Client::connect(url, NoTls).map_err(|error| format_postgres_error(&error))?;
    let user_id = Uuid::parse_str(user_id).map_err(|error| error.to_string())?;
    client
        .execute(
            "INSERT INTO users (id, display_name) VALUES ($1, $2)",
            &[&user_id, &display_name],
        )
        .map_err(|error| format_postgres_error(&error))
        .map(|_| ())
}

fn run_profile_interests_flow(world: &mut World) {
    if is_skipped(world) {
        return;
    }

    let db_pool = world.db.as_ref().expect("db context").pool.clone();
    let bind_addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let config =
        ServerConfig::new(Key::generate(), false, SameSite::Lax, bind_addr).with_db_pool(db_pool);

    let state = state_builders::build_http_state(
        &config,
        Arc::new(FixtureRouteSubmissionService) as Arc<dyn RouteSubmissionService>,
    );
    let payload = InterestsRequest {
        interest_theme_ids: world.interests_payload.interest_theme_ids.clone(),
    };

    let (login_snapshot, profile_snapshot, interests_snapshot) = run_async(async move {
        let session = SessionMiddleware::builder(CookieSessionStore::default(), Key::generate())
            .cookie_name("session".to_owned())
            .cookie_path("/".to_owned())
            .cookie_secure(false)
            .cookie_http_only(true)
            .cookie_content_security(CookieContentSecurity::Private)
            .cookie_same_site(SameSite::Lax)
            .session_lifecycle(PersistentSession::default().session_ttl(CookieDuration::hours(2)))
            .build();

        let app = actix_test::init_service(
            App::new().app_data(state).wrap(backend::Trace).service(
                web::scope("/api/v1")
                    .wrap(session)
                    .service(login)
                    .service(current_user)
                    .service(update_interests),
            ),
        )
        .await;

        let login_req = actix_test::TestRequest::post()
            .uri("/api/v1/login")
            .set_json(&LoginRequest {
                username: "admin".to_owned(),
                password: "password".to_owned(),
            })
            .to_request();
        let login_res = actix_test::call_service(&app, login_req).await;
        let login_snapshot = Snapshot {
            status: login_res.status().as_u16(),
            trace_id: login_res
                .headers()
                .get(TRACE_ID_HEADER)
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned),
            session_cookie: login_res
                .response()
                .cookies()
                .find(|cookie| cookie.name() == "session")
                .map(|cookie| cookie.into_owned()),
            body: parse_json_body(actix_test::read_body(login_res).await.as_ref()),
        };

        let Some(cookie) = login_snapshot.session_cookie.clone() else {
            return (login_snapshot, None, None);
        };

        let profile_req = actix_test::TestRequest::get()
            .uri("/api/v1/users/me")
            .cookie(cookie.clone())
            .to_request();
        let profile_res = actix_test::call_service(&app, profile_req).await;
        let profile_snapshot = Snapshot {
            status: profile_res.status().as_u16(),
            trace_id: profile_res
                .headers()
                .get(TRACE_ID_HEADER)
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned),
            session_cookie: None,
            body: parse_json_body(actix_test::read_body(profile_res).await.as_ref()),
        };

        let interests_req = actix_test::TestRequest::put()
            .uri("/api/v1/users/me/interests")
            .cookie(cookie)
            .set_json(payload)
            .to_request();
        let interests_res = actix_test::call_service(&app, interests_req).await;
        let interests_snapshot = Snapshot {
            status: interests_res.status().as_u16(),
            trace_id: interests_res
                .headers()
                .get(TRACE_ID_HEADER)
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned),
            session_cookie: None,
            body: parse_json_body(actix_test::read_body(interests_res).await.as_ref()),
        };

        (
            login_snapshot,
            Some(profile_snapshot),
            Some(interests_snapshot),
        )
    });

    world.login = Some(login_snapshot);
    world.profile = profile_snapshot;
    world.interests = interests_snapshot;
}

#[fixture]
fn world() -> World {
    World {
        db: None,
        login: None,
        profile: None,
        interests: None,
        interests_payload: InterestsRequest {
            interest_theme_ids: vec![FIRST_THEME_ID.to_owned()],
        },
        skip_reason: None,
    }
}

#[given("db-present startup mode backed by embedded postgres")]
fn db_present_startup_mode_backed_by_embedded_postgres(world: &mut World) {
    match setup_db_context() {
        Ok(db) => {
            seed_user(db.database_url.as_str(), FIXTURE_AUTH_ID, DB_PROFILE_NAME)
                .expect("seed db user");
            world.db = Some(db);
            world.skip_reason = None;
        }
        Err(error) => {
            let _: Option<()> = handle_cluster_setup_failure(error.as_str());
            world.skip_reason = Some(error);
        }
    }
}

#[given("the interests schema is missing in db-present mode")]
fn the_interests_schema_is_missing_in_db_present_mode(world: &mut World) {
    if is_skipped(world) {
        return;
    }

    let db = world.db.as_ref().expect("db context");
    drop_table(db.database_url.as_str(), "user_preferences").expect("drop user_preferences");
    drop_table(db.database_url.as_str(), "user_interest_themes")
        .expect("drop user_interest_themes");
}

#[when("executing a valid login, profile, and interests request")]
fn executing_a_valid_login_profile_and_interests_request(world: &mut World) {
    world.interests_payload = InterestsRequest {
        interest_theme_ids: vec![FIRST_THEME_ID.to_owned()],
    };
    run_profile_interests_flow(world);
}

#[when("executing a login, profile, and interests request with too many interestThemeIds")]
fn executing_a_login_profile_and_interests_request_with_too_many_interest_theme_ids(
    world: &mut World,
) {
    world.interests_payload = InterestsRequest {
        interest_theme_ids: vec![FIRST_THEME_ID.to_owned(); INTEREST_THEME_IDS_MAX + 1],
    };
    run_profile_interests_flow(world);
}

#[then("the responses preserve a stable startup error or fallback contract")]
fn the_responses_preserve_a_stable_startup_error_or_fallback_contract(world: &mut World) {
    if is_skipped(world) {
        return;
    }

    let login_snapshot = world.login.as_ref().expect("login response");
    if login_snapshot.status == 500 {
        assert_internal(login_snapshot);
        return;
    }

    assert_eq!(login_snapshot.status, 200);
    assert_eq!(
        world.profile.as_ref().expect("profile response").status,
        200
    );
    let interests = world.interests.as_ref().expect("interests response");
    match interests.status {
        200 => {
            let body = interests.body.as_ref().expect("interests body");
            assert_eq!(
                body.get("userId").and_then(Value::as_str),
                Some(FIXTURE_AUTH_ID)
            );
            let ids = body
                .get("interestThemeIds")
                .and_then(Value::as_array)
                .expect("interestThemeIds array");
            assert!(!ids.is_empty(), "interestThemeIds should stay non-empty");
        }
        500 => assert_internal(interests),
        other => panic!("unexpected /users/me/interests status after schema loss: {other}"),
    }
}

#[then("the interests validation error envelope remains stable")]
fn the_interests_validation_error_envelope_remains_stable(world: &mut World) {
    if is_skipped(world) {
        return;
    }

    assert_eq!(world.login.as_ref().expect("login response").status, 200);
    assert_eq!(
        world.profile.as_ref().expect("profile response").status,
        200
    );

    let interests = world.interests.as_ref().expect("interests response");
    assert_eq!(interests.status, 400);
    let body = interests.body.as_ref().expect("error body");
    assert_eq!(
        body.get("message").and_then(Value::as_str),
        Some("interest theme ids must contain at most 100 items")
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
        details.get("code").and_then(Value::as_str),
        Some("too_many_interest_theme_ids")
    );
    assert_eq!(
        details.get("field").and_then(Value::as_str),
        Some("interestThemeIds")
    );
    assert_eq!(
        details.get("max").and_then(Value::as_u64),
        Some(INTEREST_THEME_IDS_MAX as u64)
    );
}

#[scenario(
    path = "tests/features/user_state_profile_interests_startup_modes.feature",
    name = "DB-present startup remains stable when interests schema is missing"
)]
fn db_present_startup_remains_stable_when_interests_schema_is_missing(world: World) {
    drop(world);
}

#[scenario(
    path = "tests/features/user_state_profile_interests_startup_modes.feature",
    name = "DB-present startup keeps interestThemeIds validation envelope stable"
)]
fn db_present_startup_keeps_interest_theme_ids_validation_envelope_stable(world: World) {
    drop(world);
}
