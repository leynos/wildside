//! BDD-style step definitions for adapter guardrails.
//!
//! The `rstest-bdd` step macros register these functions for feature-based
//! tests, but we also call the functions directly from Rust tests to keep the
//! suite easy to read and refactor.
//
// rstest-bdd generates guard variables with double underscores, which trips
// the non_snake_case lint under -D warnings.
#![allow(non_snake_case)]

use actix_web::http::header;
use actix_web_actors::ws::CloseCode;
use awc::{ws::Frame, ws::Message};
use backend::domain::{
    DisplayName, Error, User, UserCreatedEvent, UserEvent, UserId,
};
use backend::inbound::http::users::LoginRequest;
use futures_util::{SinkExt, StreamExt};
use rstest_bdd_macros::{given, then, when};
use serde_json::Value;
use uuid::Uuid;

use crate::doubles::LoginResponse;
pub(crate) use crate::doubles::UsersResponse;
use crate::harness::{with_world_async, SharedWorld};
use backend::TraceId;

async fn perform_ws_submission(
    base_url: String,
    payload: Value,
) -> (Option<Value>, Option<CloseCode>) {
    let (_resp, mut socket) = awc::Client::default()
        .ws(format!("{base_url}/ws"))
        .set_header(header::ORIGIN, "http://localhost:3000")
        .connect()
        .await
        .expect("websocket connect");

    socket
        .send(Message::Text(payload.to_string().into()))
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
}

fn perform_login_request(
    world: &SharedWorld,
    username: &str,
    password: &str,
    mock_response: Option<LoginResponse>,
) {
    if let Some(response) = mock_response {
        let login = { world.borrow().login.clone() };
        login.set_response(response);
    }

    let payload = LoginRequest {
        username: username.to_owned(),
        password: password.to_owned(),
    };

    let (status, cookie_header) = with_world_async(world, |base_url| async move {
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

#[given("a running server wired with mocked HTTP and WS ports")]
pub(crate) fn a_running_server_wired_with_mocked_http_and_ws_ports(_world: SharedWorld) {}

#[when("the client logs in with valid credentials")]
pub(crate) fn the_client_logs_in_with_valid_credentials(world: SharedWorld) {
    perform_login_request(&world, "admin", "password", None);
}

#[when("the client logs in with invalid credentials")]
pub(crate) fn the_client_logs_in_with_invalid_credentials(world: SharedWorld) {
    let error_response = LoginResponse::Err(Error::unauthorized("invalid credentials"));
    perform_login_request(&world, "admin", "wrong", Some(error_response));
}

#[when("the client requests the users list")]
pub(crate) fn the_client_requests_the_users_list(world: SharedWorld) {
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

#[when("the client requests the users list without a valid session")]
pub(crate) fn the_client_requests_the_users_list_without_a_valid_session(world: SharedWorld) {
    let status = with_world_async(&world, |base_url| async move {
        let response = awc::Client::default()
            .get(format!("{base_url}/api/v1/users"))
            .send()
            .await
            .expect("users request");
        response.status().as_u16()
    });

    let mut ctx = world.borrow_mut();
    ctx.last_status = Some(status);
}

#[then("the HTTP response is success and a session cookie is set")]
pub(crate) fn the_http_response_is_success_and_a_session_cookie_is_set(world: SharedWorld) {
    let ctx = world.borrow();
    assert_eq!(ctx.last_status, Some(200));
    let cookie = ctx.session_cookie.as_deref().expect("cookie present");
    assert!(
        cookie.starts_with("session="),
        "expected session cookie, got: {cookie}"
    );
}

#[then("the HTTP response is unauthorised")]
pub(crate) fn the_http_response_is_unauthorised(world: SharedWorld) {
    let ctx = world.borrow();
    assert_eq!(ctx.last_status, Some(401));
}

#[then("the HTTP response is unauthorised and no session cookie is set")]
pub(crate) fn the_http_response_is_unauthorised_and_no_session_cookie_is_set(world: SharedWorld) {
    let ctx = world.borrow();
    assert_eq!(ctx.last_status, Some(401));
    assert!(
        ctx.session_cookie.is_none(),
        "expected no Set-Cookie header on unauthorised responses"
    );
}

#[then("the login port was called with the expected credentials")]
pub(crate) fn the_login_port_was_called_with_the_expected_credentials(world: SharedWorld) {
    let ctx = world.borrow();
    assert_eq!(
        ctx.login.calls(),
        vec![("admin".to_owned(), "password".to_owned())]
    );
}

#[then("the users port was called with the authenticated user id")]
pub(crate) fn the_users_port_was_called_with_the_authenticated_user_id(world: SharedWorld) {
    let ctx = world.borrow();
    assert_eq!(
        ctx.users.calls(),
        vec!["11111111-1111-1111-1111-111111111111".to_owned()]
    );
}

#[then("the users port is not called")]
pub(crate) fn the_users_port_is_not_called(world: SharedWorld) {
    let ctx = world.borrow();
    assert_eq!(ctx.users.calls(), Vec::<String>::new());
}

#[then("the users response includes the expected display name")]
pub(crate) fn the_users_response_includes_the_expected_display_name(world: SharedWorld) {
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

#[when("the client connects to the WebSocket and submits a display name")]
pub(crate) fn the_client_connects_to_the_websocket_and_submits_a_display_name(world: SharedWorld) {
    let event = UserEvent::UserCreated(UserCreatedEvent {
        trace_id: TraceId::from_uuid(Uuid::nil()),
        user: User::new(
            UserId::new("33333333-3333-3333-3333-333333333333").expect("fixture user id"),
            DisplayName::new("Bob").expect("fixture display name"),
        ),
    });

    let onboarding = { world.borrow().onboarding.clone() };
    onboarding.push_response(event);

    let (json, close) = with_world_async(&world, |base_url| async move {
        let payload = serde_json::json!({
            "traceId": Uuid::nil(),
            "displayName": "Bob"
        });
        perform_ws_submission(base_url, payload).await
    });

    let mut ctx = world.borrow_mut();
    ctx.last_ws_value = json;
    ctx.last_ws_close = close;
}

#[then("the WebSocket response is a user created event and the port is called once")]
pub(crate) fn the_websocket_response_is_a_user_created_event_and_the_port_is_called_once(
    world: SharedWorld,
) {
    let ctx = world.borrow();
    assert_eq!(ctx.onboarding.calls(), vec![(Uuid::nil(), "Bob".to_owned())]);
    let json = ctx.last_ws_value.as_ref().expect("ws json");
    assert_eq!(json.get("displayName").and_then(Value::as_str), Some("Bob"));
    assert_eq!(
        json.get("id").and_then(Value::as_str),
        Some("33333333-3333-3333-3333-333333333333")
    );
}

#[when("the client sends malformed JSON over the WebSocket")]
pub(crate) fn the_client_sends_malformed_json_over_the_websocket(world: SharedWorld) {
    let baseline = {
        let ctx = world.borrow();
        ctx.onboarding.calls().len()
    };

    let close_code = with_world_async(&world, |base_url| async move {
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
        match frame {
            Frame::Close(reason) => reason.map(|r| r.code).unwrap_or(CloseCode::Normal),
            other => panic!("expected close frame, got {other:?}"),
        }
    });

    let mut ctx = world.borrow_mut();
    ctx.last_ws_close = Some(close_code);
    ctx.last_ws_call_count_baseline = Some(baseline);
}

#[then("the socket closes with a policy error and the port is not called")]
pub(crate) fn the_socket_closes_with_a_policy_error_and_the_port_is_not_called(world: SharedWorld) {
    let ctx = world.borrow();
    assert_eq!(ctx.last_ws_close, Some(CloseCode::Policy));
    let baseline = ctx
        .last_ws_call_count_baseline
        .expect("ws call baseline recorded");
    assert_eq!(ctx.onboarding.calls().len(), baseline);
}
