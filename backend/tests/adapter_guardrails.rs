//! Integration guardrails for inbound adapters (HTTP + WebSocket).
//!
//! These tests exercise real Actix handlers over real sockets while substituting
//! deterministic driving ports. This ensures adapters stay side-effect free and
//! domain logic remains framework-agnostic.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::net::TcpListener;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use actix_session::config::{CookieContentSecurity, PersistentSession};
use actix_session::storage::CookieSessionStore;
use actix_session::SessionMiddleware;
use actix_web::cookie::{time::Duration as CookieDuration, Key, SameSite};
use actix_web::dev::ServerHandle;
use actix_web::http::header;
use actix_web::{web, App, HttpServer};
use actix_web_actors::ws::CloseCode;
use async_trait::async_trait;
use awc::{ws::Frame, ws::Message};
use backend::domain::ports::{LoginService, UserOnboarding, UsersQuery};
use backend::domain::{
    DisplayName, DisplayNameRejectedEvent, Error, ErrorCode, LoginCredentials, User,
    UserCreatedEvent, UserEvent, UserId, UserValidationError,
};
use backend::inbound::http::state::HttpState;
use backend::inbound::http::users::LoginRequest;
use backend::inbound::http::users::{list_users as list_users_handler, login as login_handler};
use backend::inbound::ws;
use backend::inbound::ws::state::WsState;
use backend::TraceId;
use futures_util::{SinkExt, StreamExt};
use rstest::{fixture, rstest};
use rstest_bdd_macros::{given, then, when};
use serde_json::Value;
use tokio::runtime::Runtime;
use tokio::task::LocalSet;
use uuid::Uuid;

// -----------------------------------------------------------------------------
// Test doubles for driving ports
// -----------------------------------------------------------------------------

#[derive(Clone)]
enum LoginResponse {
    Ok(UserId),
    Err(Error),
}

#[derive(Clone)]
enum UsersResponse {
    Ok(Vec<User>),
    Err(Error),
}

#[derive(Clone)]
struct RecordingLoginService {
    calls: Arc<Mutex<Vec<(String, String)>>>,
    response: Arc<Mutex<LoginResponse>>,
}

impl RecordingLoginService {
    fn new(response: LoginResponse) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            response: Arc::new(Mutex::new(response)),
        }
    }

    fn calls(&self) -> Vec<(String, String)> {
        self.calls.lock().expect("login calls lock").clone()
    }
}

#[async_trait]
impl LoginService for RecordingLoginService {
    async fn authenticate(&self, credentials: &LoginCredentials) -> Result<UserId, Error> {
        self.calls.lock().expect("login calls lock").push((
            credentials.username().to_owned(),
            credentials.password().to_owned(),
        ));
        match self.response.lock().expect("login response lock").clone() {
            LoginResponse::Ok(user_id) => Ok(user_id),
            LoginResponse::Err(error) => Err(error),
        }
    }
}

#[derive(Clone)]
struct RecordingUsersQuery {
    calls: Arc<Mutex<Vec<String>>>,
    response: Arc<Mutex<UsersResponse>>,
}

impl RecordingUsersQuery {
    fn new(response: UsersResponse) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            response: Arc::new(Mutex::new(response)),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("users calls lock").clone()
    }
}

#[async_trait]
impl UsersQuery for RecordingUsersQuery {
    async fn list_users(&self, authenticated_user: &UserId) -> Result<Vec<User>, Error> {
        self.calls
            .lock()
            .expect("users calls lock")
            .push(authenticated_user.to_string());
        match self.response.lock().expect("users response lock").clone() {
            UsersResponse::Ok(users) => Ok(users),
            UsersResponse::Err(error) => Err(error),
        }
    }
}

#[derive(Clone)]
struct QueueUserOnboarding {
    calls: Arc<Mutex<Vec<(Uuid, String)>>>,
    responses: Arc<Mutex<VecDeque<UserEvent>>>,
}

impl QueueUserOnboarding {
    fn new(responses: Vec<UserEvent>) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            responses: Arc::new(Mutex::new(VecDeque::from(responses))),
        }
    }

    fn calls(&self) -> Vec<(Uuid, String)> {
        self.calls.lock().expect("ws calls lock").clone()
    }
}

impl UserOnboarding for QueueUserOnboarding {
    fn register(&self, trace_id: TraceId, display_name: String) -> UserEvent {
        self.calls
            .lock()
            .expect("ws calls lock")
            .push((*trace_id.as_uuid(), display_name));
        self.responses
            .lock()
            .expect("ws responses lock")
            .pop_front()
            .expect("ws response queue should contain an event")
    }
}

// -----------------------------------------------------------------------------
// Server harness
// -----------------------------------------------------------------------------

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
            .service(list_users_handler);

        App::new()
            .app_data(http_data.clone())
            .app_data(ws_data.clone())
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

// -----------------------------------------------------------------------------
// World
// -----------------------------------------------------------------------------

struct AdapterWorld {
    runtime: Runtime,
    local: LocalSet,
    base_url: String,
    server: ServerHandle,
    login: RecordingLoginService,
    users: RecordingUsersQuery,
    onboarding: QueueUserOnboarding,
    last_status: Option<u16>,
    last_body: Option<Value>,
    session_cookie: Option<String>,
    last_ws_value: Option<Value>,
    last_ws_close: Option<CloseCode>,
    last_ws_call_count_baseline: Option<usize>,
}

type SharedWorld = Rc<RefCell<AdapterWorld>>;

fn shutdown(world: SharedWorld) {
    // `LocalSet` must be driven on the thread that owns it, so we lock the world
    // while calling `block_on`. The future must not try to lock the world.
    let ctx = world.borrow();
    let server = ctx.server.clone();
    ctx.local.block_on(&ctx.runtime, async move {
        server.stop(true).await;
    });
}

fn with_world_async<R, F>(world: &SharedWorld, operation: impl FnOnce(String) -> F) -> R
where
    F: std::future::Future<Output = R>,
{
    let ctx = world.borrow();
    let base_url = ctx.base_url.clone();
    ctx.local.block_on(&ctx.runtime, operation(base_url))
}

#[fixture]
fn world() -> SharedWorld {
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
    let onboarding = QueueUserOnboarding::new(Vec::new());

    let http_state = HttpState::new(Arc::new(login.clone()), Arc::new(users.clone()));
    let ws_state = WsState::new(Arc::new(onboarding.clone()));

    let (base_url, server) = local
        .block_on(&runtime, async {
            spawn_adapter_server(http_state, ws_state).await
        })
        .expect("server should start");

    Rc::new(RefCell::new(AdapterWorld {
        runtime,
        local,
        base_url,
        server,
        login,
        users,
        onboarding,
        last_status: None,
        last_body: None,
        session_cookie: None,
        last_ws_value: None,
        last_ws_close: None,
        last_ws_call_count_baseline: None,
    }))
}

// -----------------------------------------------------------------------------
// BDD step definitions (synchronous; async work runs via LocalSet)
// -----------------------------------------------------------------------------

#[given("a running server wired with mocked HTTP and WS ports")]
fn a_running_server_wired_with_mocked_http_and_ws_ports(_world: SharedWorld) {}

#[when("the client logs in with valid credentials")]
fn the_client_logs_in_with_valid_credentials(world: SharedWorld) {
    let payload = LoginRequest {
        username: "admin".into(),
        password: "password".into(),
    };

    let (status, cookie_header) = with_world_async(&world, |base_url| async move {
        let response = awc::Client::default()
            .post(format!("{base_url}/api/v1/login"))
            .send_json(&payload)
            .await
            .expect("login request");

        let status = response.status().as_u16();
        let cookie_header = response
            .headers()
            .get(header::SET_COOKIE)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_owned());
        (status, cookie_header)
    });

    let mut ctx = world.borrow_mut();
    ctx.last_status = Some(status);
    ctx.session_cookie = cookie_header;
}

#[when("the client requests the users list")]
fn the_client_requests_the_users_list(world: SharedWorld) {
    let cookie = {
        let ctx = world.borrow();
        ctx.session_cookie
            .clone()
            .expect("session cookie set")
            .split(';')
            .next()
            .expect("cookie pair")
            .to_owned()
    };

    let (status, json) = with_world_async(&world, |base_url| async move {
        let mut response = awc::Client::default()
            .get(format!("{base_url}/api/v1/users"))
            .insert_header((header::COOKIE, cookie))
            .send()
            .await
            .expect("users request");

        let status = response.status().as_u16();
        let body = response.body().await.expect("users body");
        let json: Value = serde_json::from_slice(&body).expect("users json");
        (status, json)
    });

    let mut ctx = world.borrow_mut();
    ctx.last_status = Some(status);
    ctx.last_body = Some(json);
}

#[then("the HTTP response is success and a session cookie is set")]
fn the_http_response_is_success_and_a_session_cookie_is_set(world: SharedWorld) {
    let ctx = world.borrow();
    assert_eq!(ctx.last_status, Some(200));
    let cookie = ctx.session_cookie.as_deref().expect("cookie present");
    assert!(
        cookie.starts_with("session="),
        "expected session cookie, got: {cookie}"
    );
}

#[then("the login port was called with the expected credentials")]
fn the_login_port_was_called_with_the_expected_credentials(world: SharedWorld) {
    let ctx = world.borrow();
    assert_eq!(
        ctx.login.calls(),
        vec![("admin".to_owned(), "password".to_owned())]
    );
}

#[then("the users port was called with the authenticated user id")]
fn the_users_port_was_called_with_the_authenticated_user_id(world: SharedWorld) {
    let ctx = world.borrow();
    assert_eq!(
        ctx.users.calls(),
        vec!["11111111-1111-1111-1111-111111111111".to_owned()]
    );
}

#[then("the users response includes the expected display name")]
fn the_users_response_includes_the_expected_display_name(world: SharedWorld) {
    let ctx = world.borrow();
    let body = ctx.last_body.as_ref().expect("users body present");
    let first = body
        .as_array()
        .expect("users array")
        .first()
        .expect("user row");
    assert_eq!(
        first.get("displayName").and_then(Value::as_str),
        Some("Ada Lovelace")
    );
}

#[when("the client logs in with invalid credentials")]
fn the_client_logs_in_with_invalid_credentials(world: SharedWorld) {
    let login = { world.borrow().login.clone() };
    *login.response.lock().expect("login response lock") =
        LoginResponse::Err(Error::unauthorized("invalid credentials"));

    let payload = LoginRequest {
        username: "admin".into(),
        password: "wrong".into(),
    };

    let status = with_world_async(&world, |base_url| async move {
        let response = awc::Client::default()
            .post(format!("{base_url}/api/v1/login"))
            .send_json(&payload)
            .await
            .expect("login request");
        response.status().as_u16()
    });

    let mut ctx = world.borrow_mut();
    ctx.last_status = Some(status);
}

#[then("the HTTP response is unauthorised and no session cookie is set")]
fn the_http_response_is_unauthorised_and_no_session_cookie_is_set(world: SharedWorld) {
    let ctx = world.borrow();
    assert_eq!(ctx.last_status, Some(401));
    assert!(ctx.session_cookie.is_none());
}

#[when("the client connects to the WebSocket and submits a display name")]
fn the_client_connects_to_the_websocket_and_submits_a_display_name(world: SharedWorld) {
    let event = UserEvent::UserCreated(UserCreatedEvent {
        trace_id: TraceId::from_uuid(Uuid::nil()),
        user: User::new(
            UserId::new("33333333-3333-3333-3333-333333333333").expect("fixture user id"),
            DisplayName::new("Bob").expect("fixture display name"),
        ),
    });

    let onboarding = { world.borrow().onboarding.clone() };
    onboarding
        .responses
        .lock()
        .expect("ws responses lock")
        .push_back(event);

    let (json, close) = with_world_async(&world, |base_url| async move {
        let (_resp, mut socket) = awc::Client::default()
            .ws(format!("{base_url}/ws"))
            .set_header(header::ORIGIN, "http://localhost:3000")
            .connect()
            .await
            .expect("websocket connect");

        let payload = serde_json::json!({
            "traceId": Uuid::nil(),
            "displayName": "Bob"
        })
        .to_string();

        socket
            .send(Message::Text(payload.into()))
            .await
            .expect("send message");

        let frame = socket.next().await.expect("ws frame").expect("ws frame ok");
        match frame {
            Frame::Text(bytes) => {
                let json: Value = serde_json::from_slice(&bytes).expect("ws json");
                (Some(json), None)
            }
            Frame::Close(reason) => {
                let code = reason.map(|r| r.code).unwrap_or(CloseCode::Normal);
                (None, Some(code))
            }
            other => panic!("unexpected ws frame: {other:?}"),
        }
    });

    let mut ctx = world.borrow_mut();
    ctx.last_ws_value = json;
    ctx.last_ws_close = close;
}

#[then("the WebSocket response is a user created event and the port is called once")]
fn the_websocket_response_is_a_user_created_event_and_the_port_is_called_once(world: SharedWorld) {
    let ctx = world.borrow();
    assert_eq!(
        ctx.onboarding.calls(),
        vec![(Uuid::nil(), "Bob".to_owned())]
    );
    let json = ctx.last_ws_value.as_ref().expect("ws json");
    assert_eq!(json.get("displayName").and_then(Value::as_str), Some("Bob"));
    assert_eq!(
        json.get("id").and_then(Value::as_str),
        Some("33333333-3333-3333-3333-333333333333")
    );
}

#[when("the client sends malformed JSON over the WebSocket")]
fn the_client_sends_malformed_json_over_the_websocket(world: SharedWorld) {
    let baseline = {
        let ctx = world.borrow();
        ctx.onboarding.calls().len()
    };

    let (baseline, close_code) = with_world_async(&world, |base_url| async move {
        let (_resp, mut socket) = awc::Client::default()
            .ws(format!("{base_url}/ws"))
            .set_header(header::ORIGIN, "http://localhost:3000")
            .connect()
            .await
            .expect("websocket connect");

        socket
            .send(Message::Text("not-json".into()))
            .await
            .expect("send message");

        let frame = socket.next().await.expect("ws frame").expect("ws frame ok");
        let close_code = match frame {
            Frame::Close(reason) => reason.map(|r| r.code).unwrap_or(CloseCode::Normal),
            other => panic!("expected close frame, got {other:?}"),
        };

        (baseline, close_code)
    });

    let mut ctx = world.borrow_mut();
    ctx.last_ws_close = Some(close_code);
    ctx.last_ws_call_count_baseline = Some(baseline);
}

#[then("the socket closes with a policy error and the port is not called")]
fn the_socket_closes_with_a_policy_error_and_the_port_is_not_called(world: SharedWorld) {
    let ctx = world.borrow();
    assert_eq!(ctx.last_ws_close, Some(CloseCode::Policy));
    let baseline = ctx
        .last_ws_call_count_baseline
        .expect("ws call baseline recorded");
    assert_eq!(ctx.onboarding.calls().len(), baseline);
}

// -----------------------------------------------------------------------------
// Behavioural tests
// -----------------------------------------------------------------------------

#[rstest]
fn http_happy_path_uses_injected_ports(world: SharedWorld) {
    a_running_server_wired_with_mocked_http_and_ws_ports(world.clone());
    the_client_logs_in_with_valid_credentials(world.clone());
    the_http_response_is_success_and_a_session_cookie_is_set(world.clone());
    the_login_port_was_called_with_the_expected_credentials(world.clone());

    the_client_requests_the_users_list(world.clone());
    the_users_port_was_called_with_the_authenticated_user_id(world.clone());
    the_users_response_includes_the_expected_display_name(world.clone());

    shutdown(world);
}

#[rstest]
fn http_unhappy_path_does_not_set_cookie(world: SharedWorld) {
    a_running_server_wired_with_mocked_http_and_ws_ports(world.clone());
    the_client_logs_in_with_invalid_credentials(world.clone());
    the_http_response_is_unauthorised_and_no_session_cookie_is_set(world.clone());

    {
        let ctx = world.borrow();
        assert_eq!(
            ctx.login.calls(),
            vec![("admin".to_owned(), "wrong".to_owned())]
        );
        assert_eq!(ctx.users.calls(), Vec::<String>::new());
    }

    shutdown(world);
}

#[rstest]
fn websocket_happy_path_uses_injected_port(world: SharedWorld) {
    a_running_server_wired_with_mocked_http_and_ws_ports(world.clone());
    the_client_connects_to_the_websocket_and_submits_a_display_name(world.clone());
    the_websocket_response_is_a_user_created_event_and_the_port_is_called_once(world.clone());

    the_client_sends_malformed_json_over_the_websocket(world.clone());
    the_socket_closes_with_a_policy_error_and_the_port_is_not_called(world.clone());

    shutdown(world);
}

// -----------------------------------------------------------------------------
// Compilation guard (documents intent)
// -----------------------------------------------------------------------------

#[test]
fn guardrails_tests_are_exercising_mocks_not_fixtures() {
    assert_eq!(Error::unauthorized("x").code(), ErrorCode::Unauthorized);
    let _ = UsersResponse::Err(Error::internal("boom"));
    let _ = DisplayNameRejectedEvent {
        trace_id: TraceId::from_uuid(Uuid::nil()),
        attempted_name: "bad".into(),
        error: UserValidationError::DisplayNameInvalidCharacters,
    };
}
