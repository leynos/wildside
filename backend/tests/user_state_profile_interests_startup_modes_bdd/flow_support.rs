//! Shared flow and assertion helpers for profile/interests startup-mode BDD.

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;

use actix_web::cookie::{Cookie, Key, SameSite};
use actix_web::{App, test as actix_test, web};
use backend::domain::TRACE_ID_HEADER;
use backend::domain::ports::{FixtureRouteSubmissionService, RouteSubmissionService};
use backend::inbound::http::users::{
    InterestsRequest, LoginRequest, current_user, login, update_interests,
};
use backend::outbound::persistence::{DbPool, PoolConfig};
use pg_embedded_setup_unpriv::TemporaryDatabase;
use postgres::{Client, NoTls};
use serde_json::Value;
use uuid::Uuid;

use super::support::atexit_cleanup::shared_cluster_handle;
use super::support::profile_interests::{FIXTURE_AUTH_ID, build_session_middleware};
use super::support::{format_postgres_error, provision_template_database};
use super::{ServerConfig, state_builders};

#[derive(Debug)]
pub(crate) struct Snapshot {
    pub(crate) status: u16,
    pub(crate) body: Option<Value>,
    pub(crate) trace_id: Option<String>,
    pub(crate) session_cookie: Option<Cookie<'static>>,
}

pub(crate) struct DbContext {
    pub(crate) database_url: String,
    pub(crate) pool: DbPool,
    _database: TemporaryDatabase,
}

pub(crate) struct World {
    pub(crate) db: Option<DbContext>,
    pub(crate) login: Option<Snapshot>,
    pub(crate) profile: Option<Snapshot>,
    pub(crate) interests: Option<Snapshot>,
    pub(crate) interests_payload: InterestsRequest,
    pub(crate) skip_reason: Option<String>,
}

fn run_async<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(future)
}

fn parse_json_body(bytes: &[u8]) -> Option<Value> {
    (!bytes.is_empty()).then(|| serde_json::from_slice(bytes).expect("json body"))
}

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

pub(crate) fn assert_interests_response(
    snapshot: &Snapshot,
    expected_ids: &[&str],
    expected_revision: u32,
) {
    assert_eq!(snapshot.status, 200);
    let body = snapshot.body.as_ref().expect("interests body");
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
    assert_eq!(
        body.get("revision").and_then(Value::as_u64),
        Some(u64::from(expected_revision))
    );
}

pub(crate) fn is_skipped(world: &World) -> bool {
    if let Some(reason) = world.skip_reason.as_deref() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped ({reason})");
        true
    } else {
        false
    }
}

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

pub(crate) fn seed_user(url: &str, user_id: Uuid, display_name: &str) -> Result<(), String> {
    let mut client = Client::connect(url, NoTls).map_err(|error| format_postgres_error(&error))?;
    client
        .execute(
            "INSERT INTO users (id, display_name) VALUES ($1, $2)",
            &[&user_id, &display_name],
        )
        .map_err(|error| format_postgres_error(&error))
        .map(|_| ())
}

type AppData<T> = web::Data<T>;
type AppState = backend::inbound::http::state::HttpState;

async fn capture_snapshot(res: actix_web::dev::ServiceResponse, with_cookie: bool) -> Snapshot {
    Snapshot {
        status: res.status().as_u16(),
        trace_id: res
            .headers()
            .get(TRACE_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned),
        session_cookie: with_cookie
            .then(|| {
                res.response()
                    .cookies()
                    .find(|cookie| cookie.name() == "session")
                    .map(|cookie| cookie.into_owned())
            })
            .flatten(),
        body: parse_json_body(actix_test::read_body(res).await.as_ref()),
    }
}

async fn execute_profile_interests_flow(
    state: AppData<AppState>,
    payload: InterestsRequest,
) -> (Snapshot, Option<Snapshot>, Option<Snapshot>) {
    let app = actix_test::init_service(
        App::new().app_data(state).wrap(backend::Trace).service(
            web::scope("/api/v1")
                .wrap(build_session_middleware())
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
    let login_snapshot = capture_snapshot(login_res, true).await;

    let Some(cookie) = login_snapshot.session_cookie.clone() else {
        return (login_snapshot, None, None);
    };

    let profile_req = actix_test::TestRequest::get()
        .uri("/api/v1/users/me")
        .cookie(cookie.clone())
        .to_request();
    let profile_res = actix_test::call_service(&app, profile_req).await;
    let profile_snapshot = capture_snapshot(profile_res, false).await;

    let interests_req = actix_test::TestRequest::put()
        .uri("/api/v1/users/me/interests")
        .cookie(cookie)
        .set_json(payload)
        .to_request();
    let interests_res = actix_test::call_service(&app, interests_req).await;
    let interests_snapshot = capture_snapshot(interests_res, false).await;

    (
        login_snapshot,
        Some(profile_snapshot),
        Some(interests_snapshot),
    )
}

pub(crate) fn run_profile_interests_flow(world: &mut World) {
    if is_skipped(world) {
        return;
    }

    let bind_addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let base_config = ServerConfig::new(Key::generate(), false, SameSite::Lax, bind_addr);
    let config = match world.db.as_ref() {
        Some(db) => base_config.with_db_pool(db.pool.clone()),
        None => base_config,
    };
    let state = state_builders::build_http_state(
        &config,
        Arc::new(FixtureRouteSubmissionService) as Arc<dyn RouteSubmissionService>,
    );
    let payload = InterestsRequest {
        interest_theme_ids: world.interests_payload.interest_theme_ids.clone(),
        expected_revision: world.interests_payload.expected_revision,
    };

    let (login_snapshot, profile_snapshot, interests_snapshot) =
        run_async(execute_profile_interests_flow(state, payload));

    world.login = Some(login_snapshot);
    world.profile = profile_snapshot;
    world.interests = interests_snapshot;
}
