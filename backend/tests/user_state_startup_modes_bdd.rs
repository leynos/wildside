//! Behaviour coverage for 3.5.2 startup modes.

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;

use actix_http::Request;
use actix_session::SessionMiddleware;
use actix_session::config::{CookieContentSecurity, PersistentSession};
use actix_session::storage::CookieSessionStore;
use actix_web::cookie::{Cookie, Key, SameSite, time::Duration as CookieDuration};
use actix_web::{
    App,
    body::BoxBody,
    dev::{Service, ServiceResponse},
    test as actix_test, web,
};
use backend::domain::TRACE_ID_HEADER;
use backend::domain::ports::{FixtureRouteSubmissionService, RouteSubmissionService};
use backend::inbound::http::state::HttpState;
use backend::inbound::http::users::{LoginRequest, list_users, login};
use backend::outbound::persistence::{DbPool, PoolConfig};
use pg_embedded_setup_unpriv::TemporaryDatabase;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use serde_json::Value;

mod support;

use support::atexit_cleanup::shared_cluster_handle;
use support::embedded_postgres::drop_users_table;
use support::{handle_cluster_setup_failure, provision_template_database};

#[expect(
    dead_code,
    reason = "server config include exposes members unused in this integration test"
)]
#[path = "../src/server/config.rs"]
mod server_config;
pub use server_config::ServerConfig;

#[path = "../src/server/state_builders.rs"]
mod state_builders;

const FIXTURE_USERS_ID: &str = "3fa85f64-5717-4562-b3fc-2c963f66afa6";
const FIXTURE_USERS_NAME: &str = "Ada Lovelace";

#[derive(Debug, Clone, Copy)]
enum Mode {
    Fixture,
    Db,
}

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
    mode: Mode,
    db: Option<DbContext>,
    login: Option<Snapshot>,
    users: Option<Snapshot>,
    skip_reason: Option<String>,
}

fn run_async<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(future)
}

fn build_http_state_for_tests(
    config: &ServerConfig,
    route_submission: Arc<dyn RouteSubmissionService>,
) -> web::Data<HttpState> {
    state_builders::build_http_state(config, route_submission)
}

fn is_fixture_users(body: &Value) -> bool {
    let users = body.as_array().expect("users array");
    users.iter().any(|user| {
        user.get("id").and_then(Value::as_str) == Some(FIXTURE_USERS_ID)
            && user.get("displayName").and_then(Value::as_str) == Some(FIXTURE_USERS_NAME)
    })
}

fn parse_json_body(bytes: &[u8]) -> Option<Value> {
    if bytes.is_empty() {
        None
    } else {
        Some(serde_json::from_slice(bytes).expect("json body"))
    }
}

async fn build_test_app_with_session(
    state: web::Data<HttpState>,
) -> impl Service<Request, Response = ServiceResponse<BoxBody>, Error = actix_web::Error> {
    let session = SessionMiddleware::builder(CookieSessionStore::default(), Key::generate())
        .cookie_name("session".to_owned())
        .cookie_path("/".to_owned())
        .cookie_secure(false)
        .cookie_http_only(true)
        .cookie_content_security(CookieContentSecurity::Private)
        .cookie_same_site(SameSite::Lax)
        .session_lifecycle(PersistentSession::default().session_ttl(CookieDuration::hours(2)))
        .build();

    actix_test::init_service(
        App::new().app_data(state).wrap(backend::Trace).service(
            web::scope("/api/v1")
                .wrap(session)
                .service(login)
                .service(list_users),
        ),
    )
    .await
}

async fn execute_and_snapshot_login<S>(app: &S, username: &str, password: &str) -> Snapshot
where
    S: Service<Request, Response = ServiceResponse<BoxBody>, Error = actix_web::Error>,
{
    let login_req = actix_test::TestRequest::post()
        .uri("/api/v1/login")
        .set_json(&LoginRequest {
            username: username.to_owned(),
            password: password.to_owned(),
        })
        .to_request();
    let login_res = actix_test::call_service(app, login_req).await;
    Snapshot {
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
    }
}

async fn execute_and_snapshot_users<S>(app: &S, cookie: Cookie<'_>) -> Snapshot
where
    S: Service<Request, Response = ServiceResponse<BoxBody>, Error = actix_web::Error>,
{
    let users_req = actix_test::TestRequest::get()
        .uri("/api/v1/users")
        .cookie(cookie)
        .to_request();
    let users_res = actix_test::call_service(app, users_req).await;
    Snapshot {
        status: users_res.status().as_u16(),
        trace_id: users_res
            .headers()
            .get(TRACE_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned),
        session_cookie: None,
        body: parse_json_body(actix_test::read_body(users_res).await.as_ref()),
    }
}

async fn run_flow(
    state: web::Data<HttpState>,
    username: &str,
    password: &str,
) -> (Snapshot, Option<Snapshot>) {
    let app = build_test_app_with_session(state).await;
    let login_snapshot = execute_and_snapshot_login(&app, username, password).await;

    let Some(cookie) = login_snapshot.session_cookie.clone() else {
        return (login_snapshot, None);
    };

    let users_snapshot = execute_and_snapshot_users(&app, cookie).await;

    (login_snapshot, Some(users_snapshot))
}

fn setup_db() -> Result<DbContext, String> {
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

fn skipped(world: &World) -> bool {
    if let Some(reason) = world.skip_reason.as_deref() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped ({reason})");
        true
    } else {
        false
    }
}

#[fixture]
fn world() -> World {
    World {
        mode: Mode::Fixture,
        db: None,
        login: None,
        users: None,
        skip_reason: None,
    }
}

#[given("fixture-fallback startup mode")]
fn fixture_fallback_startup_mode(world: &mut World) {
    world.mode = Mode::Fixture;
}

#[given("db-present startup mode backed by embedded postgres")]
fn db_present_startup_mode_backed_by_embedded_postgres(world: &mut World) {
    world.mode = Mode::Db;
    match setup_db() {
        Ok(db) => {
            world.db = Some(db);
            world.skip_reason = None;
        }
        Err(error) => {
            let _: Option<()> = handle_cluster_setup_failure(error.as_str());
            world.skip_reason = Some(error);
        }
    }
}

#[given("the users table is missing in db-present mode")]
fn the_users_table_is_missing_in_db_present_mode(world: &mut World) {
    if skipped(world) {
        return;
    }
    let db = world.db.as_ref().expect("db context");
    drop_users_table(db.database_url.as_str()).expect("drop users table");
}

fn execute_login_flow(world: &mut World, username: &str, password: &str) {
    if skipped(world) {
        return;
    }
    let pool = world.db.as_ref().map(|db| db.pool.clone());
    let bind_addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let config = match pool {
        Some(pool) => {
            ServerConfig::new(Key::generate(), false, SameSite::Lax, bind_addr).with_db_pool(pool)
        }
        None => ServerConfig::new(Key::generate(), false, SameSite::Lax, bind_addr),
    };
    let state = build_http_state_for_tests(&config, Arc::new(FixtureRouteSubmissionService));
    let (login_snapshot, users_snapshot) = run_async(run_flow(state, username, password));
    world.login = Some(login_snapshot);
    world.users = users_snapshot;
}

#[when("executing a valid login and users request")]
fn executing_a_valid_login_and_users_request(world: &mut World) {
    execute_login_flow(world, "admin", "password");
}

#[when("executing an invalid login request")]
fn executing_an_invalid_login_request(world: &mut World) {
    execute_login_flow(world, "admin", "wrong-password");
}

#[then("login succeeds with a session cookie")]
fn login_succeeds_with_a_session_cookie(world: &mut World) {
    if skipped(world) {
        return;
    }
    let login_snapshot = world.login.as_ref().expect("login response");
    assert_eq!(login_snapshot.status, 200);
    assert!(
        login_snapshot.session_cookie.is_some(),
        "session cookie should be set"
    );
}

#[then("the users response matches fixture fallback payload")]
fn the_users_response_matches_fixture_fallback_payload(world: &mut World) {
    if skipped(world) {
        return;
    }
    let users = world.users.as_ref().expect("users response");
    assert_eq!(users.status, 200);
    let body = users.body.as_ref().expect("users body");
    assert!(is_fixture_users(body), "expected fixture fallback payload");
}

#[then("the login response is unauthorised with stable error envelope")]
fn the_login_response_is_unauthorised_with_stable_error_envelope(world: &mut World) {
    if skipped(world) {
        return;
    }
    let login_snapshot = world.login.as_ref().expect("login response");
    assert_eq!(login_snapshot.status, 401);
    let body = login_snapshot.body.as_ref().expect("error body");
    assert_eq!(
        body.get("message").and_then(Value::as_str),
        Some("invalid credentials")
    );
    assert_eq!(
        body.get("code").and_then(Value::as_str),
        Some("unauthorized")
    );
}

#[then("the responses preserve a stable startup error or fallback contract")]
fn the_responses_preserve_a_stable_startup_error_or_fallback_contract(world: &mut World) {
    if skipped(world) {
        return;
    }
    let login_snapshot = world.login.as_ref().expect("login response");
    if login_snapshot.status == 500 {
        let body = login_snapshot.body.as_ref().expect("error body");
        assert_eq!(
            body.get("message").and_then(Value::as_str),
            Some("Internal server error")
        );
        assert_eq!(
            body.get("code").and_then(Value::as_str),
            Some("internal_error")
        );
        assert_eq!(
            body.get("traceId").and_then(Value::as_str),
            login_snapshot.trace_id.as_deref()
        );
        return;
    }

    assert_eq!(login_snapshot.status, 200);
    let users = world.users.as_ref().expect("users response");
    match users.status {
        200 => assert!(is_fixture_users(users.body.as_ref().expect("users body"))),
        500 => {
            let body = users.body.as_ref().expect("error body");
            assert_eq!(
                body.get("message").and_then(Value::as_str),
                Some("Internal server error")
            );
            assert_eq!(
                body.get("code").and_then(Value::as_str),
                Some("internal_error")
            );
            assert_eq!(
                body.get("traceId").and_then(Value::as_str),
                users.trace_id.as_deref()
            );
        }
        other => panic!("unexpected /users status after schema loss: {other}"),
    }
}

#[scenario(
    path = "tests/features/user_state_startup_modes.feature",
    name = "Fixture fallback startup keeps fixture login and users behaviour"
)]
fn fixture_fallback_startup_keeps_fixture_login_and_users_behaviour(world: World) {
    drop(world);
}

#[scenario(
    path = "tests/features/user_state_startup_modes.feature",
    name = "DB-present startup rejects invalid credentials"
)]
fn db_present_startup_rejects_invalid_credentials(world: World) {
    drop(world);
}

#[scenario(
    path = "tests/features/user_state_startup_modes.feature",
    name = "DB-present startup remains stable when users schema is missing"
)]
fn db_present_startup_remains_stable_when_users_schema_is_missing(world: World) {
    drop(world);
}
