//! Flow helpers for users list pagination BDD coverage.

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;

use actix_web::body::BoxBody;
use actix_web::cookie::{Cookie, Key, SameSite};
use actix_web::dev::{Service, ServiceResponse};
use actix_web::{App, test as actix_test, web};
use backend::domain::ports::{FixtureRouteSubmissionService, RouteSubmissionService};
use backend::inbound::http::state::HttpState;
use backend::inbound::http::users::{LoginRequest, list_users, login};
use backend::outbound::persistence::{DbPool, PoolConfig};
use backend::test_support::server::{ServerConfig, build_http_state};
use pg_embedded_setup_unpriv::TemporaryDatabase;
use postgres::{Client, NoTls};
use serde_json::Value;
use url::Url;
use uuid::Uuid;

use super::support::atexit_cleanup::shared_cluster_handle;
use super::support::profile_interests::build_session_middleware;
use super::support::{format_postgres_error, provision_template_database};

const ADMIN_USER_ID: &str = "123e4567-e89b-12d3-a456-426614174000";
pub(crate) const ORDERED_USER_IDS: [&str; 5] = [
    ADMIN_USER_ID,
    "123e4567-e89b-12d3-a456-426614174001",
    "123e4567-e89b-12d3-a456-426614174002",
    "123e4567-e89b-12d3-a456-426614174003",
    "123e4567-e89b-12d3-a456-426614174004",
];

pub(crate) struct DbContext {
    database_url: String,
    pool: DbPool,
    _database: TemporaryDatabase,
}

#[derive(Default)]
pub(crate) struct World {
    db: Option<DbContext>,
    last_response: Option<Snapshot>,
    traversal_ids: Vec<String>,
    skip_reason: Option<String>,
}

#[derive(Clone, Debug)]
struct Snapshot {
    status: u16,
    body: Option<Value>,
}

pub(crate) fn run_async<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(future)
}

pub(crate) fn is_skipped(world: &World) -> bool {
    if let Some(reason) = world.skip_reason.as_deref() {
        eprintln!("SKIP-TEST-CLUSTER: users list pagination scenario skipped ({reason})");
        true
    } else {
        false
    }
}

fn with_world<F: FnOnce(&mut World)>(world: &mut World, f: F) {
    if !is_skipped(world) {
        f(world);
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

pub(crate) fn seed_users(db: DbContext) -> Result<DbContext, String> {
    let mut client = Client::connect(db.database_url.as_str(), NoTls)
        .map_err(|error| format_postgres_error(&error))?;
    for (index, id) in ORDERED_USER_IDS.iter().enumerate() {
        let user_id = Uuid::parse_str(id).expect("fixture user id");
        let display_name = format!("Page User {}", index + 1);
        let created_at = format!("2026-01-01T00:0{index}:00Z");
        client
            .execute(
                "INSERT INTO users (id, display_name, created_at)
                 VALUES ($1, $2, ($3::text)::timestamptz)",
                &[&user_id, &display_name, &created_at],
            )
            .map_err(|error| format_postgres_error(&error))?;
    }
    Ok(db)
}

pub(crate) fn store_db(world: &mut World, db: DbContext) {
    world.db = Some(db);
    world.skip_reason = None;
}

pub(crate) fn skip(world: &mut World, reason: String) {
    world.skip_reason = Some(reason);
}

fn build_state(pool: DbPool) -> web::Data<HttpState> {
    let bind_addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let config =
        ServerConfig::new(Key::generate(), false, SameSite::Lax, bind_addr).with_db_pool(pool);
    build_http_state(
        &config,
        Arc::new(FixtureRouteSubmissionService) as Arc<dyn RouteSubmissionService>,
    )
}

async fn build_app(
    state: web::Data<HttpState>,
) -> impl Service<actix_http::Request, Response = ServiceResponse<BoxBody>, Error = actix_web::Error>
{
    actix_test::init_service(
        App::new().app_data(state).wrap(backend::Trace).service(
            web::scope("/api/v1")
                .wrap(build_session_middleware())
                .service(login)
                .service(list_users),
        ),
    )
    .await
}

async fn login_cookie<S>(app: &S) -> Cookie<'static>
where
    S: Service<actix_http::Request, Response = ServiceResponse<BoxBody>, Error = actix_web::Error>,
{
    let request = actix_test::TestRequest::post()
        .uri("/api/v1/login")
        .set_json(&LoginRequest {
            username: "admin".to_owned(),
            password: "password".to_owned(),
        })
        .to_request();
    let response = actix_test::call_service(app, request).await;
    assert_eq!(response.status().as_u16(), 200);
    response
        .response()
        .cookies()
        .find(|cookie| cookie.name() == "session")
        .expect("session cookie")
        .into_owned()
}

async fn get_users<S>(app: &S, path: &str, cookie: Option<Cookie<'static>>) -> Snapshot
where
    S: Service<actix_http::Request, Response = ServiceResponse<BoxBody>, Error = actix_web::Error>,
{
    let mut request = actix_test::TestRequest::get().uri(path);
    if let Some(cookie) = cookie {
        request = request.cookie(cookie);
    }
    let response = actix_test::call_service(app, request.to_request()).await;
    Snapshot {
        status: response.status().as_u16(),
        body: parse_json_body(actix_test::read_body(response).await.as_ref()),
    }
}

async fn collect_pages_until_final<S>(app: &S, cookie: Cookie<'static>) -> (Snapshot, Vec<String>)
where
    S: Service<actix_http::Request, Response = ServiceResponse<BoxBody>, Error = actix_web::Error>,
{
    let mut path = "/api/v1/users?limit=2".to_owned();
    let mut traversal_ids = Vec::new();
    for _ in 0..10 {
        let snapshot = get_users(app, &path, Some(cookie.clone())).await;
        traversal_ids.extend(user_ids(&snapshot));
        match next_path(&snapshot) {
            Some(page_path) => path = page_path,
            None => return (snapshot, traversal_ids),
        }
    }
    panic!("pagination traversal did not terminate");
}

fn parse_json_body(bytes: &[u8]) -> Option<Value> {
    (!bytes.is_empty()).then(|| serde_json::from_slice(bytes).expect("json body"))
}

fn build_path_from_link(link: &str) -> String {
    let url = Url::parse(link).expect("pagination link should be absolute URL");
    match url.query() {
        Some(query) => format!("{}?{query}", url.path()),
        None => url.path().to_owned(),
    }
}

fn link(snapshot: &Snapshot, rel: &str) -> Option<String> {
    snapshot
        .body
        .as_ref()
        .and_then(|body| body.get("links"))
        .and_then(|links| links.get(rel))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn user_ids(snapshot: &Snapshot) -> Vec<String> {
    snapshot
        .body
        .as_ref()
        .and_then(|body| body.get("data"))
        .and_then(Value::as_array)
        .expect("users data array")
        .iter()
        .map(|user| {
            user.get("id")
                .and_then(Value::as_str)
                .expect("user id")
                .to_owned()
        })
        .collect()
}

fn error_detail_code(snapshot: &Snapshot) -> Option<&str> {
    snapshot
        .body
        .as_ref()
        .and_then(|body| body.get("details"))
        .and_then(|details| details.get("code"))
        .and_then(Value::as_str)
}

fn next_path(snapshot: &Snapshot) -> Option<String> {
    link(snapshot, "next").map(|next| build_path_from_link(&next))
}

pub(crate) fn run_first_page(world: &mut World) {
    with_world(world, |world| {
        let db = world.db.as_ref().expect("db context");
        world.last_response = Some(run_async(async {
            let app = build_app(build_state(db.pool.clone())).await;
            let cookie = login_cookie(&app).await;
            get_users(&app, "/api/v1/users?limit=2", Some(cookie)).await
        }));
    });
}

pub(crate) fn run_follow_next_to_final(world: &mut World) {
    with_world(world, |world| {
        let db = world.db.as_ref().expect("db context");
        let (last_response, traversal_ids) = run_async(async {
            let app = build_app(build_state(db.pool.clone())).await;
            let cookie = login_cookie(&app).await;
            collect_pages_until_final(&app, cookie).await
        });
        world.last_response = Some(last_response);
        world.traversal_ids = traversal_ids;
    });
}

pub(crate) fn run_next_then_prev(world: &mut World) {
    with_world(world, |world| {
        let db = world.db.as_ref().expect("db context");
        world.last_response = Some(run_async(async {
            let app = build_app(build_state(db.pool.clone())).await;
            let cookie = login_cookie(&app).await;
            let first = get_users(&app, "/api/v1/users?limit=2", Some(cookie.clone())).await;
            let middle_path =
                build_path_from_link(&link(&first, "next").expect("first page next link"));
            let middle = get_users(&app, &middle_path, Some(cookie.clone())).await;
            let final_path =
                build_path_from_link(&link(&middle, "next").expect("middle page next link"));
            let final_page = get_users(&app, &final_path, Some(cookie.clone())).await;
            let prev_path =
                build_path_from_link(&link(&final_page, "prev").expect("final page prev link"));
            get_users(&app, &prev_path, Some(cookie)).await
        }));
    });
}

pub(crate) fn run_authenticated_request(world: &mut World, path: &'static str) {
    with_world(world, |world| {
        let db = world.db.as_ref().expect("db context");
        world.last_response = Some(run_async(async {
            let app = build_app(build_state(db.pool.clone())).await;
            let cookie = login_cookie(&app).await;
            get_users(&app, path, Some(cookie)).await
        }));
    });
}

pub(crate) fn run_unauthenticated_request(world: &mut World) {
    with_world(world, |world| {
        let db = world.db.as_ref().expect("db context");
        world.last_response = Some(run_async(async {
            let app = build_app(build_state(db.pool.clone())).await;
            get_users(&app, "/api/v1/users", None).await
        }));
    });
}

pub(crate) fn assert_status(world: &mut World, status: u16) {
    with_world(world, |world| {
        assert_eq!(
            world.last_response.as_ref().expect("response").status,
            status
        );
    });
}

pub(crate) fn assert_users(world: &mut World, expected: &[&str]) {
    with_world(world, |world| {
        let ids = user_ids(world.last_response.as_ref().expect("response"));
        assert_eq!(ids, expected);
    });
}

pub(crate) fn assert_next_only(world: &mut World) {
    with_world(world, |world| {
        let response = world.last_response.as_ref().expect("response");
        assert!(link(response, "next").is_some());
        assert!(link(response, "prev").is_none());
    });
}

pub(crate) fn assert_prev_only(world: &mut World) {
    with_world(world, |world| {
        let response = world.last_response.as_ref().expect("response");
        assert!(link(response, "prev").is_some());
        assert!(link(response, "next").is_none());
    });
}

pub(crate) fn assert_full_traversal(world: &mut World) {
    with_world(world, |world| {
        assert_eq!(world.traversal_ids, ORDERED_USER_IDS)
    });
}

pub(crate) fn assert_error(world: &mut World, status: u16, detail_code: &str) {
    with_world(world, |world| {
        let response = world.last_response.as_ref().expect("response");
        assert_eq!(response.status, status);
        assert_eq!(error_detail_code(response), Some(detail_code));
    });
}
