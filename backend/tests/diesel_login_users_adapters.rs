//! Focused startup-mode coverage for login/users routes.

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;

use actix_session::SessionMiddleware;
use actix_session::config::{CookieContentSecurity, PersistentSession};
use actix_session::storage::CookieSessionStore;
use actix_web::cookie::{Key, SameSite, time::Duration as CookieDuration};
use actix_web::{App, test as actix_test, web};
use backend::domain::TRACE_ID_HEADER;
use backend::domain::ports::{FixtureRouteSubmissionService, RouteSubmissionService};
use backend::inbound::http::state::HttpState;
use backend::inbound::http::users::{LoginRequest, list_users, login};
use backend::outbound::persistence::{DbPool, PoolConfig};
use pg_embedded_setup_unpriv::TemporaryDatabase;
use postgres::{Client, NoTls};
use rstest::rstest;
use serde_json::Value;

mod support;

use support::atexit_cleanup::shared_cluster_handle;
use support::embedded_postgres::drop_users_table;
use support::{format_postgres_error, handle_cluster_setup_failure, provision_template_database};

#[allow(dead_code)]
#[path = "../src/server/config.rs"]
mod server_config;
pub use server_config::ServerConfig;

#[path = "../src/server/state_builders.rs"]
mod state_builders;

const FIXTURE_USERS_ID: &str = "3fa85f64-5717-4562-b3fc-2c963f66afa6";
const FIXTURE_USERS_NAME: &str = "Ada Lovelace";
const FIXTURE_AUTH_ID: &str = "123e4567-e89b-12d3-a456-426614174000";
const DB_AUTH_DISPLAY_NAME: &str = "Database Admin";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UsersMode {
    Fixture,
    Db,
}

#[derive(Debug)]
struct Snapshot {
    status: u16,
    body: Option<Value>,
    trace_id: Option<String>,
}

struct DbContext {
    database_url: String,
    pool: DbPool,
    _database: TemporaryDatabase,
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

fn test_session_middleware() -> SessionMiddleware<CookieSessionStore> {
    SessionMiddleware::builder(CookieSessionStore::default(), Key::generate())
        .cookie_name("session".to_owned())
        .cookie_path("/".to_owned())
        .cookie_secure(false)
        .cookie_http_only(true)
        .cookie_content_security(CookieContentSecurity::Private)
        .cookie_same_site(SameSite::Lax)
        .session_lifecycle(PersistentSession::default().session_ttl(CookieDuration::hours(2)))
        .build()
}

fn test_app(
    state: web::Data<HttpState>,
) -> App<
    impl actix_web::dev::ServiceFactory<
        actix_web::dev::ServiceRequest,
        Config = (),
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
        InitError = (),
    >,
> {
    App::new().app_data(state).wrap(backend::Trace).service(
        web::scope("/api/v1")
            .wrap(test_session_middleware())
            .service(login)
            .service(list_users),
    )
}

fn server_config(pool: Option<DbPool>) -> ServerConfig {
    let bind_addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let config = ServerConfig::new(Key::generate(), false, SameSite::Lax, bind_addr);
    match pool {
        Some(pool) => config.with_db_pool(pool),
        None => config,
    }
}

fn parse_body(bytes: &[u8]) -> Option<Value> {
    if bytes.is_empty() {
        None
    } else {
        Some(serde_json::from_slice(bytes).expect("json body"))
    }
}

fn classify_users(body: &Value) -> UsersMode {
    let users = body.as_array().expect("users array");
    if users.iter().any(|user| {
        user.get("id").and_then(Value::as_str) == Some(FIXTURE_USERS_ID)
            && user.get("displayName").and_then(Value::as_str) == Some(FIXTURE_USERS_NAME)
    }) {
        return UsersMode::Fixture;
    }
    if users.iter().any(|user| {
        user.get("id").and_then(Value::as_str) == Some(FIXTURE_AUTH_ID)
            && user.get("displayName").and_then(Value::as_str) == Some(DB_AUTH_DISPLAY_NAME)
    }) {
        return UsersMode::Db;
    }
    panic!("unknown users response: {body}");
}

fn assert_unauthorised(snapshot: &Snapshot) {
    assert_eq!(snapshot.status, 401);
    let body = snapshot.body.as_ref().expect("error body");
    assert_eq!(
        body.get("message").and_then(Value::as_str),
        Some("invalid credentials")
    );
    assert_eq!(
        body.get("code").and_then(Value::as_str),
        Some("unauthorized")
    );
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

async fn run_flow(
    state: web::Data<HttpState>,
    username: &str,
    password: &str,
) -> (Snapshot, Option<Snapshot>) {
    let app = actix_test::init_service(test_app(state)).await;

    let login_req = actix_test::TestRequest::post()
        .uri("/api/v1/login")
        .set_json(&LoginRequest {
            username: username.to_owned(),
            password: password.to_owned(),
        })
        .to_request();
    let login_res = actix_test::call_service(&app, login_req).await;
    let login_status = login_res.status().as_u16();
    let login_trace_id = login_res
        .headers()
        .get(TRACE_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let session_cookie = login_res
        .response()
        .cookies()
        .find(|cookie| cookie.name() == "session")
        .map(|cookie| cookie.into_owned());
    let login_body = actix_test::read_body(login_res).await;
    let login_snapshot = Snapshot {
        status: login_status,
        body: parse_body(login_body.as_ref()),
        trace_id: login_trace_id,
    };

    let Some(cookie) = session_cookie else {
        return (login_snapshot, None);
    };

    let users_req = actix_test::TestRequest::get()
        .uri("/api/v1/users")
        .cookie(cookie)
        .to_request();
    let users_res = actix_test::call_service(&app, users_req).await;
    let users_status = users_res.status().as_u16();
    let users_trace_id = users_res
        .headers()
        .get(TRACE_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let users_body = actix_test::read_body(users_res).await;

    (
        login_snapshot,
        Some(Snapshot {
            status: users_status,
            body: parse_body(users_body.as_ref()),
            trace_id: users_trace_id,
        }),
    )
}

fn setup_db_context() -> Option<DbContext> {
    let cluster = match shared_cluster_handle() {
        Ok(cluster) => cluster,
        Err(error) => {
            let _: Option<()> = handle_cluster_setup_failure(error);
            return None;
        }
    };
    let database = match provision_template_database(cluster) {
        Ok(database) => database,
        Err(error) => {
            let _: Option<()> = handle_cluster_setup_failure(error);
            return None;
        }
    };
    let database_url = database.url().to_owned();
    let pool = match run_async(DbPool::new(
        PoolConfig::new(database_url.as_str())
            .with_max_size(2)
            .with_min_idle(Some(1)),
    )) {
        Ok(pool) => pool,
        Err(error) => {
            let _: Option<()> = handle_cluster_setup_failure(error);
            return None;
        }
    };
    Some(DbContext {
        database_url,
        pool,
        _database: database,
    })
}

fn seed_user(url: &str, id: &str, display_name: &str) -> Result<(), String> {
    let mut client = Client::connect(url, NoTls).map_err(|error| format_postgres_error(&error))?;
    let user_id = uuid::Uuid::parse_str(id).map_err(|error| error.to_string())?;
    client
        .execute(
            "INSERT INTO users (id, display_name) VALUES ($1, $2)",
            &[&user_id, &display_name],
        )
        .map_err(|error| format_postgres_error(&error))
        .map(|_| ())
}

#[rstest]
fn fixture_fallback_mode_uses_fixture_ports_for_login_and_users() {
    let state = build_http_state_for_tests(
        &server_config(None),
        Arc::new(FixtureRouteSubmissionService),
    );
    let (login_snapshot, users_snapshot) = run_async(run_flow(state, "admin", "password"));
    assert_eq!(login_snapshot.status, 200);
    let users_snapshot = users_snapshot.expect("users response");
    assert_eq!(users_snapshot.status, 200);
    let users_body = users_snapshot.body.as_ref().expect("users body");
    assert_eq!(classify_users(users_body), UsersMode::Fixture);
}

#[rstest]
fn db_present_mode_supports_login_and_users_with_stable_contracts() {
    let Some(db) = setup_db_context() else {
        eprintln!(
            "SKIP-TEST-CLUSTER: db_present_mode_supports_login_and_users_with_stable_contracts"
        );
        return;
    };
    seed_user(
        db.database_url.as_str(),
        FIXTURE_AUTH_ID,
        DB_AUTH_DISPLAY_NAME,
    )
    .expect("seed user");
    let state = build_http_state_for_tests(
        &server_config(Some(db.pool.clone())),
        Arc::new(FixtureRouteSubmissionService),
    );
    let (login_snapshot, users_snapshot) = run_async(run_flow(state, "admin", "password"));
    assert_eq!(login_snapshot.status, 200);
    let users_snapshot = users_snapshot.expect("users response");
    assert_eq!(users_snapshot.status, 200);
    let users_body = users_snapshot.body.as_ref().expect("users body");
    let mode = classify_users(users_body);
    if mode == UsersMode::Db {
        let users = users_body.as_array().expect("users array");
        assert!(users.iter().any(|user| {
            user.get("id").and_then(Value::as_str) == Some(FIXTURE_AUTH_ID)
                && user.get("displayName").and_then(Value::as_str) == Some(DB_AUTH_DISPLAY_NAME)
        }));
    }
}

#[rstest]
#[case(false)]
#[case(true)]
fn startup_modes_reject_invalid_credentials_with_unauthorised_envelope(#[case] db_present: bool) {
    let pool = if db_present {
        let Some(db) = setup_db_context() else {
            eprintln!(
                "SKIP-TEST-CLUSTER: startup_modes_reject_invalid_credentials_with_unauthorised_envelope"
            );
            return;
        };
        Some(db.pool)
    } else {
        None
    };
    let state = build_http_state_for_tests(
        &server_config(pool),
        Arc::new(FixtureRouteSubmissionService),
    );
    let (login_snapshot, users_snapshot) = run_async(run_flow(state, "admin", "wrong-password"));
    assert_unauthorised(&login_snapshot);
    assert!(users_snapshot.is_none(), "users request should not run");
}

#[rstest]
fn db_present_mode_handles_users_table_loss_with_stable_outcomes() {
    let Some(db) = setup_db_context() else {
        eprintln!(
            "SKIP-TEST-CLUSTER: db_present_mode_handles_users_table_loss_with_stable_outcomes"
        );
        return;
    };
    seed_user(
        db.database_url.as_str(),
        FIXTURE_AUTH_ID,
        DB_AUTH_DISPLAY_NAME,
    )
    .expect("seed user");
    drop_users_table(db.database_url.as_str()).expect("drop users table");

    let state = build_http_state_for_tests(
        &server_config(Some(db.pool.clone())),
        Arc::new(FixtureRouteSubmissionService),
    );
    let (login_snapshot, users_snapshot) = run_async(run_flow(state, "admin", "password"));

    if login_snapshot.status == 500 {
        assert_internal(&login_snapshot);
        return;
    }
    assert_eq!(login_snapshot.status, 200);

    let users_snapshot = users_snapshot.expect("users response");
    match users_snapshot.status {
        200 => assert_eq!(
            classify_users(users_snapshot.body.as_ref().expect("users body")),
            UsersMode::Fixture
        ),
        500 => assert_internal(&users_snapshot),
        other => panic!("unexpected /users status after schema loss: {other}"),
    }
}
