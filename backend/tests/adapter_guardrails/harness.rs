//! Server harness and shared world for adapter guardrails.
//!
//! The harness owns a single-threaded Tokio runtime plus a `LocalSet` because
//! Actix uses `spawn_local` internally. The `WorldFixture` ensures the server
//! is stopped even if a test panics.

use std::cell::RefCell;
use std::net::TcpListener;
use std::rc::Rc;
use std::sync::Arc;

use actix_session::SessionMiddleware;
use actix_session::config::{CookieContentSecurity, PersistentSession};
use actix_session::storage::CookieSessionStore;
use actix_web::cookie::{Key, SameSite, time::Duration as CookieDuration};
use actix_web::dev::ServerHandle;
use actix_web::{App, HttpServer, web};
use actix_ws::CloseCode;
use rstest::fixture;
use serde_json::Value;
use tokio::runtime::Runtime;
use tokio::task::LocalSet;

use crate::doubles::{
    LoginResponse, QueueUserOnboarding, RecordingLoginService, RecordingUserInterestsCommand,
    RecordingUserProfileQuery, RecordingUsersQuery, UserInterestsResponse, UserProfileResponse,
    UsersResponse,
};
use backend::Trace;
use backend::domain::ports::FixtureRouteSubmissionService;
use backend::domain::{DisplayName, InterestThemeId, User, UserId, UserInterests};
use backend::inbound::http::state::{HttpState, HttpStatePorts};
use backend::inbound::http::users::{
    current_user as current_user_handler, list_users as list_users_handler, login as login_handler,
    update_interests as update_interests_handler,
};
use backend::inbound::ws;
use backend::inbound::ws::state::WsState;

pub(crate) struct AdapterWorld {
    pub(crate) runtime: Runtime,
    pub(crate) local: LocalSet,
    pub(crate) base_url: String,
    pub(crate) server: ServerHandle,
    pub(crate) login: RecordingLoginService,
    pub(crate) users: RecordingUsersQuery,
    pub(crate) profile: RecordingUserProfileQuery,
    pub(crate) interests: RecordingUserInterestsCommand,
    pub(crate) onboarding: QueueUserOnboarding,
    pub(crate) last_status: Option<u16>,
    pub(crate) last_body: Option<Value>,
    pub(crate) last_trace_id: Option<String>,
    pub(crate) session_cookie: Option<String>,
    pub(crate) last_ws_value: Option<Value>,
    pub(crate) last_ws_close: Option<CloseCode>,
    pub(crate) last_ws_call_count_baseline: Option<usize>,
}

pub(crate) type SharedWorld = Rc<RefCell<AdapterWorld>>;

pub(crate) struct WorldFixture {
    world: SharedWorld,
}

impl WorldFixture {
    pub(crate) fn world(&self) -> SharedWorld {
        self.world.clone()
    }
}

impl Drop for WorldFixture {
    fn drop(&mut self) {
        shutdown(self.world.clone());
    }
}

pub(crate) fn shutdown(world: SharedWorld) {
    // `LocalSet` must be driven on the thread that owns it, so we lock the world
    // while calling `block_on`. The future must not try to lock the world.
    let ctx = world.borrow();
    let server = ctx.server.clone();
    ctx.local.block_on(&ctx.runtime, async move {
        server.stop(true).await;
    });
}

pub(crate) fn with_world_async<R, F>(world: &SharedWorld, operation: impl FnOnce(String) -> F) -> R
where
    F: std::future::Future<Output = R>,
{
    let ctx = world.borrow();
    let base_url = ctx.base_url.clone();
    ctx.local.block_on(&ctx.runtime, operation(base_url))
}

fn test_session_middleware(key: Key) -> SessionMiddleware<CookieSessionStore> {
    SessionMiddleware::builder(CookieSessionStore::default(), key)
        .cookie_name("session".to_owned())
        .cookie_path("/".to_owned())
        .cookie_secure(false)
        .cookie_http_only(true)
        .cookie_content_security(CookieContentSecurity::Private)
        .cookie_same_site(SameSite::Lax)
        .session_lifecycle(PersistentSession::default().session_ttl(CookieDuration::hours(2)))
        .build()
}

async fn spawn_adapter_server(
    http_state: HttpState,
    ws_state: WsState,
) -> Result<(String, ServerHandle), String> {
    let key = Key::generate();
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|err| err.to_string())?;
    let addr = listener.local_addr().map_err(|err| err.to_string())?;

    let http_data = web::Data::new(http_state);
    let ws_data = web::Data::new(ws_state);

    let server = HttpServer::new(move || {
        let api = web::scope("/api/v1")
            .wrap(test_session_middleware(key.clone()))
            .service(login_handler)
            .service(list_users_handler)
            .service(current_user_handler)
            .service(update_interests_handler);

        App::new()
            .app_data(http_data.clone())
            .app_data(ws_data.clone())
            .wrap(Trace)
            .service(api)
            .service(ws::ws_entry)
    })
    .disable_signals()
    .workers(1)
    .listen(listener)
    .map_err(|err| err.to_string())?
    .run();

    let handle = server.handle();
    actix_web::rt::spawn(server);

    Ok((format!("http://{addr}"), handle))
}

#[fixture]
pub(crate) fn world() -> WorldFixture {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let local = LocalSet::new();

    let login = RecordingLoginService::new(LoginResponse::Ok(
        UserId::new("11111111-1111-1111-1111-111111111111").expect("fixture user id"),
    ));
    let users = RecordingUsersQuery::new(UsersResponse::Ok(vec![User::new(
        UserId::new("22222222-2222-2222-2222-222222222222").expect("fixture user id"),
        DisplayName::new("Ada Lovelace").expect("fixture display name"),
    )]));
    let profile = RecordingUserProfileQuery::new(UserProfileResponse::Ok(User::new(
        UserId::new("11111111-1111-1111-1111-111111111111").expect("fixture user id"),
        DisplayName::new("Ada Lovelace").expect("fixture display name"),
    )));
    let interests =
        RecordingUserInterestsCommand::new(UserInterestsResponse::Ok(UserInterests::new(
            UserId::new("11111111-1111-1111-1111-111111111111").expect("fixture user id"),
            vec![
                InterestThemeId::new("3fa85f64-5717-4562-b3fc-2c963f66afa6")
                    .expect("fixture interest theme id"),
            ],
        )));
    let onboarding = QueueUserOnboarding::new(Vec::new());

    let http_state = HttpState::new(HttpStatePorts {
        login: Arc::new(login.clone()),
        users: Arc::new(users.clone()),
        profile: Arc::new(profile.clone()),
        interests: Arc::new(interests.clone()),
        route_submission: Arc::new(FixtureRouteSubmissionService),
    });
    let ws_state = crate::ws_support::ws_state(onboarding.clone());

    let (base_url, server) = local
        .block_on(&runtime, async {
            spawn_adapter_server(http_state, ws_state).await
        })
        .expect("server should start");

    let world = Rc::new(RefCell::new(AdapterWorld {
        runtime,
        local,
        base_url,
        server,
        login,
        users,
        profile,
        interests,
        onboarding,
        last_status: None,
        last_body: None,
        last_trace_id: None,
        session_cookie: None,
        last_ws_value: None,
        last_ws_close: None,
        last_ws_call_count_baseline: None,
    }));

    WorldFixture { world }
}
