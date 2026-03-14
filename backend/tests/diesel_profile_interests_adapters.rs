//! Focused startup-mode coverage for profile/interests routes.

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;

use actix_web::cookie::{Cookie, Key, SameSite};
use actix_web::{App, test as actix_test, web};
use backend::domain::ports::{FixtureRouteSubmissionService, RouteSubmissionService};
use backend::inbound::http::state::HttpState;
use backend::inbound::http::users::{
    InterestsRequest, LoginRequest, current_user, login, update_interests,
};
use backend::outbound::persistence::{DbPool, PoolConfig};
use pg_embedded_setup_unpriv::TemporaryDatabase;
use postgres::{Client, NoTls};
use rstest::rstest;
use serde_json::Value;
use uuid::Uuid;

mod support;

use support::atexit_cleanup::shared_cluster_handle;
use support::profile_interests::{
    DB_PROFILE_NAME, FIRST_THEME_ID, FIXTURE_AUTH_ID, FIXTURE_PROFILE_NAME, SECOND_THEME_ID,
    build_session_middleware,
};
use support::{format_postgres_error, handle_cluster_setup_failure, provision_template_database};

#[expect(
    dead_code,
    reason = "server config include exposes members unused in this integration test"
)]
#[path = "../src/server/config.rs"]
mod server_config;
pub use server_config::ServerConfig;

#[path = "../src/server/state_builders.rs"]
mod state_builders;

#[derive(Debug)]
struct Snapshot {
    status: u16,
    body: Option<Value>,
    session_cookie: Option<Cookie<'static>>,
}

struct DbContext {
    database_url: String,
    pool: DbPool,
    _database: TemporaryDatabase,
}

struct Credentials<'a> {
    username: &'a str,
    password: &'a str,
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

async fn build_test_app(
    state: web::Data<HttpState>,
) -> impl actix_web::dev::Service<
    actix_http::Request,
    Response = actix_web::dev::ServiceResponse<actix_web::body::BoxBody>,
    Error = actix_web::Error,
> {
    actix_test::init_service(
        App::new().app_data(state).wrap(backend::Trace).service(
            web::scope("/api/v1")
                .wrap(build_session_middleware())
                .service(login)
                .service(current_user)
                .service(update_interests),
        ),
    )
    .await
}

async fn do_login<S>(app: &S, creds: &Credentials<'_>) -> Snapshot
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse<actix_web::body::BoxBody>,
            Error = actix_web::Error,
        >,
{
    let login_req = actix_test::TestRequest::post()
        .uri("/api/v1/login")
        .set_json(&LoginRequest {
            username: creds.username.to_owned(),
            password: creds.password.to_owned(),
        })
        .to_request();
    let login_res = actix_test::call_service(app, login_req).await;

    Snapshot {
        status: login_res.status().as_u16(),
        session_cookie: login_res
            .response()
            .cookies()
            .find(|cookie| cookie.name() == "session")
            .map(|cookie| cookie.into_owned()),
        body: parse_body(actix_test::read_body(login_res).await.as_ref()),
    }
}

async fn call_and_capture<S>(app: &S, req: actix_http::Request) -> Snapshot
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse<actix_web::body::BoxBody>,
            Error = actix_web::Error,
        >,
{
    let res = actix_test::call_service(app, req).await;
    Snapshot {
        status: res.status().as_u16(),
        session_cookie: None,
        body: parse_body(actix_test::read_body(res).await.as_ref()),
    }
}

async fn do_get_profile<S>(app: &S, cookie: Cookie<'static>) -> Snapshot
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse<actix_web::body::BoxBody>,
            Error = actix_web::Error,
        >,
{
    let req = actix_test::TestRequest::get()
        .uri("/api/v1/users/me")
        .cookie(cookie)
        .to_request();
    call_and_capture(app, req).await
}

async fn do_update_interests<S>(
    app: &S,
    cookie: Cookie<'static>,
    payload: &InterestsRequest,
) -> Snapshot
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse<actix_web::body::BoxBody>,
            Error = actix_web::Error,
        >,
{
    let req = actix_test::TestRequest::put()
        .uri("/api/v1/users/me/interests")
        .cookie(cookie)
        .set_json(payload)
        .to_request();
    call_and_capture(app, req).await
}

async fn run_flow(
    state: web::Data<HttpState>,
    creds: Credentials<'_>,
    interests_payload: &InterestsRequest,
) -> (Snapshot, Option<Snapshot>, Option<Snapshot>) {
    let app = build_test_app(state).await;
    let login_snapshot = do_login(&app, &creds).await;

    let Some(cookie) = login_snapshot.session_cookie.clone() else {
        return (login_snapshot, None, None);
    };

    let profile_snapshot = do_get_profile(&app, cookie.clone()).await;
    let interests_snapshot = do_update_interests(&app, cookie, interests_payload).await;

    (
        login_snapshot,
        Some(profile_snapshot),
        Some(interests_snapshot),
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

fn seed_user(url: &str, id: Uuid, display_name: &str) -> Result<(), String> {
    let mut client = Client::connect(url, NoTls).map_err(|error| format_postgres_error(&error))?;
    client
        .execute(
            "INSERT INTO users (id, display_name) VALUES ($1, $2)",
            &[&id, &display_name],
        )
        .map_err(|error| format_postgres_error(&error))
        .map(|_| ())
}

fn db_contains_interest_id(url: &str, user_id: Uuid, theme_id: Uuid) -> Result<bool, String> {
    let mut client = Client::connect(url, NoTls).map_err(|error| format_postgres_error(&error))?;

    client
        .query_one(
            "SELECT EXISTS (
                SELECT 1 FROM user_preferences
                WHERE user_id = $1 AND $2 = ANY(interest_theme_ids)
            ) OR EXISTS (
                SELECT 1 FROM user_interest_themes
                WHERE user_id = $1 AND theme_id = $2
            )",
            &[&user_id, &theme_id],
        )
        .map_err(|error| format_postgres_error(&error))
        .map(|row| row.get::<_, bool>(0))
}

fn assert_profile_response(body: &Value, expected_display_name: &str) {
    assert_eq!(
        body.get("id").and_then(Value::as_str),
        Some(FIXTURE_AUTH_ID)
    );
    assert_eq!(
        body.get("displayName").and_then(Value::as_str),
        Some(expected_display_name)
    );
}

fn assert_interests_response(body: &Value, expected_ids: &[&str]) {
    assert_eq!(
        body.get("userId").and_then(Value::as_str),
        Some(FIXTURE_AUTH_ID)
    );
    assert_eq!(
        body.get("interestThemeIds")
            .and_then(Value::as_array)
            .expect("interestThemeIds array")
            .iter()
            .map(|value| value.as_str().expect("string interest id"))
            .collect::<Vec<_>>(),
        expected_ids
    );
    assert_eq!(body.get("revision").and_then(Value::as_u64), Some(1));
}

#[rstest]
fn fixture_fallback_mode_returns_fixture_profile_and_interests_shape() {
    let interests_payload = InterestsRequest {
        interest_theme_ids: vec![FIRST_THEME_ID.to_owned()],
        expected_revision: None,
    };
    let state = build_http_state_for_tests(
        &server_config(None),
        Arc::new(FixtureRouteSubmissionService),
    );

    let (login_snapshot, profile_snapshot, interests_snapshot) = run_async(run_flow(
        state,
        Credentials {
            username: "admin",
            password: "password",
        },
        &interests_payload,
    ));

    assert_eq!(login_snapshot.status, 200);

    let profile = profile_snapshot.expect("profile response");
    assert_eq!(profile.status, 200);
    let profile_body = profile.body.as_ref().expect("profile body");
    assert_profile_response(profile_body, FIXTURE_PROFILE_NAME);

    let interests = interests_snapshot.expect("interests response");
    assert_eq!(interests.status, 200);
    let interests_body = interests.body.as_ref().expect("interests body");
    assert_interests_response(interests_body, &[FIRST_THEME_ID]);
}

#[rstest]
fn db_present_mode_returns_db_backed_profile_and_interests_behaviour() {
    let Some(db) = setup_db_context() else {
        eprintln!(
            "SKIP-TEST-CLUSTER: db_present_mode_returns_db_backed_profile_and_interests_behaviour"
        );
        return;
    };

    seed_user(
        db.database_url.as_str(),
        Uuid::parse_str(FIXTURE_AUTH_ID).expect("valid fixture UUID"),
        DB_PROFILE_NAME,
    )
    .expect("seed user");

    let interests_payload = InterestsRequest {
        interest_theme_ids: vec![FIRST_THEME_ID.to_owned(), SECOND_THEME_ID.to_owned()],
        expected_revision: None,
    };
    let state = build_http_state_for_tests(
        &server_config(Some(db.pool.clone())),
        Arc::new(FixtureRouteSubmissionService),
    );

    let (login_snapshot, profile_snapshot, interests_snapshot) = run_async(run_flow(
        state,
        Credentials {
            username: "admin",
            password: "password",
        },
        &interests_payload,
    ));

    assert_eq!(login_snapshot.status, 200);
    let profile_body = profile_snapshot
        .expect("profile response")
        .body
        .expect("profile body");
    assert_profile_response(&profile_body, DB_PROFILE_NAME);

    let interests_body = interests_snapshot
        .expect("interests response")
        .body
        .expect("interests body");
    assert_interests_response(&interests_body, &[FIRST_THEME_ID, SECOND_THEME_ID]);

    let persisted = db_contains_interest_id(
        db.database_url.as_str(),
        Uuid::parse_str(FIXTURE_AUTH_ID).expect("valid fixture UUID"),
        Uuid::parse_str(FIRST_THEME_ID).expect("valid fixture UUID"),
    )
    .expect("query interests persistence");
    assert!(
        persisted,
        "expected interests to persist through DB-backed wiring"
    );
}
