//! Shared flow and assertion helpers for revision-safe interests BDD coverage.

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;

use actix_web::cookie::{Cookie, Key, SameSite};
use actix_web::{App, test as actix_test, web};
use backend::domain::ports::{FixtureRouteSubmissionService, RouteSubmissionService};
use backend::inbound::http::preferences::get_preferences;
use backend::inbound::http::state::HttpState;
use backend::inbound::http::users::{InterestsRequest, LoginRequest, login, update_interests};
use backend::outbound::persistence::DbPool;
use serde_json::Value;

use super::support::profile_interests::{FIXTURE_AUTH_ID, build_session_middleware};
use super::{ServerConfig, build_http_state as build_server_http_state};

mod db_support;

pub(crate) use self::db_support::{
    SeedPreferences, World, is_skipped, seed_preferences, seed_user, setup_db_context,
};

pub(crate) const FIRST_THEME_ID: &str = "3fa85f64-5717-4562-b3fc-2c963f66afa6";
pub(crate) const SECOND_THEME_ID: &str = "3fa85f64-5717-4562-b3fc-2c963f66afa7";
pub(crate) const THIRD_THEME_ID: &str = "3fa85f64-5717-4562-b3fc-2c963f66afa9";
pub(crate) const SAFETY_TOGGLE_ID: &str = "7fa85f64-5717-4562-b3fc-2c963f66afa6";

#[derive(Debug)]
pub(crate) struct Snapshot {
    pub(crate) status: u16,
    pub(crate) body: Option<Value>,
    pub(crate) session_cookie: Option<Cookie<'static>>,
}

pub(crate) struct ExpectedPreferences<'a> {
    pub(crate) interest_ids: &'a [&'a str],
    pub(crate) safety_ids: &'a [&'a str],
    pub(crate) unit_system: &'a str,
    pub(crate) revision: u32,
}

pub(crate) fn run_async<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(future)
}

fn parse_json_body(bytes: &[u8]) -> Option<Value> {
    (!bytes.is_empty()).then(|| serde_json::from_slice(bytes).expect("json body"))
}

async fn capture_snapshot(
    res: actix_web::dev::ServiceResponse<actix_web::body::BoxBody>,
    with_cookie: bool,
) -> Snapshot {
    Snapshot {
        status: res.status().as_u16(),
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

async fn call_endpoint<S>(app: &S, req: actix_http::Request) -> Snapshot
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse<actix_web::body::BoxBody>,
            Error = actix_web::Error,
        >,
{
    capture_snapshot(actix_test::call_service(app, req).await, false).await
}

async fn build_app(
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
                .service(update_interests)
                .service(get_preferences),
        ),
    )
    .await
}

fn build_http_state(pool: DbPool) -> web::Data<HttpState> {
    let bind_addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let config =
        ServerConfig::new(Key::generate(), false, SameSite::Lax, bind_addr).with_db_pool(pool);
    build_server_http_state(
        &config,
        Arc::new(FixtureRouteSubmissionService) as Arc<dyn RouteSubmissionService>,
    )
}

async fn login_cookie<S>(app: &S) -> Cookie<'static>
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
            username: "admin".to_owned(),
            password: "password".to_owned(),
        })
        .to_request();
    let login_res = actix_test::call_service(app, login_req).await;
    let snapshot = capture_snapshot(login_res, true).await;
    snapshot.session_cookie.expect("session cookie")
}

async fn update_interests_snapshot<S>(
    app: &S,
    cookie: Cookie<'static>,
    payload: InterestsRequest,
) -> Snapshot
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse<actix_web::body::BoxBody>,
            Error = actix_web::Error,
        >,
{
    call_endpoint(
        app,
        actix_test::TestRequest::put()
            .uri("/api/v1/users/me/interests")
            .cookie(cookie)
            .set_json(payload)
            .to_request(),
    )
    .await
}

async fn preferences_snapshot<S>(app: &S, cookie: Cookie<'static>) -> Snapshot
where
    S: actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse<actix_web::body::BoxBody>,
            Error = actix_web::Error,
        >,
{
    call_endpoint(
        app,
        actix_test::TestRequest::get()
            .uri("/api/v1/users/me/preferences")
            .cookie(cookie)
            .to_request(),
    )
    .await
}

fn run_single_update(world: &mut World, payload: InterestsRequest) {
    if is_skipped(world) {
        return;
    }

    let db = world.db.as_ref().expect("db context");
    let state = build_http_state(db.pool.clone());
    world.first_update = Some(run_async(async {
        let app = build_app(state).await;
        let cookie = login_cookie(&app).await;
        update_interests_snapshot(&app, cookie, payload).await
    }));
}

fn run_single_interests_update(world: &mut World, payload: InterestsRequest) {
    run_single_update(world, payload);
}

fn with_app_session<F, Fut, T>(world: &mut World, f: F) -> Option<T>
where
    F: FnOnce(DbPool) -> Fut,
    Fut: Future<Output = T>,
{
    if is_skipped(world) {
        return None;
    }

    let db = world.db.as_ref().expect("db context");
    Some(run_async(f(db.pool.clone())))
}

pub(crate) fn run_first_write(world: &mut World) {
    run_single_interests_update(
        world,
        InterestsRequest {
            interest_theme_ids: vec![FIRST_THEME_ID.to_owned()],
            expected_revision: None,
        },
    );
}

pub(crate) fn run_matching_revision_write(world: &mut World) {
    if let Some((first_update, second_update)) = with_app_session(world, |pool| async move {
        let state = build_http_state(pool);
        let app = build_app(state).await;
        let cookie = login_cookie(&app).await;
        let first = update_interests_snapshot(
            &app,
            cookie.clone(),
            InterestsRequest {
                interest_theme_ids: vec![FIRST_THEME_ID.to_owned()],
                expected_revision: None,
            },
        )
        .await;
        let second = update_interests_snapshot(
            &app,
            cookie,
            InterestsRequest {
                interest_theme_ids: vec![SECOND_THEME_ID.to_owned()],
                expected_revision: Some(1),
            },
        )
        .await;
        (first, second)
    }) {
        world.first_update = Some(first_update);
        world.second_update = Some(second_update);
    }
}

pub(crate) fn run_stale_revision_conflict(world: &mut World) {
    run_single_interests_update(
        world,
        InterestsRequest {
            interest_theme_ids: vec![SECOND_THEME_ID.to_owned()],
            expected_revision: Some(1),
        },
    );
}

pub(crate) fn run_missing_revision_conflict(world: &mut World) {
    run_single_interests_update(
        world,
        InterestsRequest {
            interest_theme_ids: vec![SECOND_THEME_ID.to_owned()],
            expected_revision: None,
        },
    );
}

pub(crate) fn run_preserve_non_interest_flow(world: &mut World) {
    if let Some((update, preferences)) = with_app_session(world, |pool| async move {
        let state = build_http_state(pool);
        let app = build_app(state).await;
        let cookie = login_cookie(&app).await;
        let update = update_interests_snapshot(
            &app,
            cookie.clone(),
            InterestsRequest {
                interest_theme_ids: vec![THIRD_THEME_ID.to_owned()],
                expected_revision: Some(1),
            },
        )
        .await;
        let preferences = preferences_snapshot(&app, cookie).await;
        (update, preferences)
    }) {
        world.first_update = Some(update);
        world.preferences = Some(preferences);
    }
}

pub(crate) fn assert_interests_snapshot(
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

pub(crate) fn assert_conflict_snapshot(
    snapshot: &Snapshot,
    expected_revision: Option<u32>,
    actual_revision: u32,
) {
    assert_eq!(snapshot.status, 409);
    let body = snapshot.body.as_ref().expect("error body");
    assert_eq!(body.get("code").and_then(Value::as_str), Some("conflict"));
    assert_eq!(
        body.get("message").and_then(Value::as_str),
        Some("revision mismatch")
    );
    let details = body
        .get("details")
        .and_then(Value::as_object)
        .expect("details object");
    assert_eq!(
        details.get("code").and_then(Value::as_str),
        Some("revision_mismatch")
    );
    match expected_revision {
        Some(expected_revision) => assert_eq!(
            details.get("expectedRevision").and_then(Value::as_u64),
            Some(u64::from(expected_revision))
        ),
        None => assert!(
            details
                .get("expectedRevision")
                .is_some_and(serde_json::Value::is_null)
        ),
    }
    assert_eq!(
        details.get("actualRevision").and_then(Value::as_u64),
        Some(u64::from(actual_revision))
    );
}

pub(crate) fn assert_preferences_snapshot(snapshot: &Snapshot, expected: ExpectedPreferences<'_>) {
    assert_eq!(snapshot.status, 200);
    let body = snapshot.body.as_ref().expect("preferences body");
    assert_eq!(
        body.get("interestThemeIds")
            .and_then(Value::as_array)
            .expect("interestThemeIds array")
            .iter()
            .map(|value| value.as_str().expect("string interest id"))
            .collect::<Vec<_>>(),
        expected.interest_ids
    );
    assert_eq!(
        body.get("safetyToggleIds")
            .and_then(Value::as_array)
            .expect("safetyToggleIds array")
            .iter()
            .map(|value| value.as_str().expect("string safety id"))
            .collect::<Vec<_>>(),
        expected.safety_ids
    );
    assert_eq!(
        body.get("unitSystem").and_then(Value::as_str),
        Some(expected.unit_system)
    );
    assert_eq!(
        body.get("revision").and_then(Value::as_u64),
        Some(u64::from(expected.revision))
    );
}
