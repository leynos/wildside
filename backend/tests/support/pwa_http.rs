//! Shared HTTP helpers for PWA preferences and annotations tests.

use actix_web::http::{Method, header};
use awc::Client;
use backend::domain::TRACE_ID_HEADER;
use backend::inbound::http::idempotency::IDEMPOTENCY_KEY_HEADER;
use serde_json::Value;

use crate::harness::{SharedWorld, with_world_async};

pub(crate) struct JsonRequest<'a> {
    pub(crate) include_cookie: bool,
    pub(crate) method: Method,
    pub(crate) path: &'a str,
    pub(crate) payload: Option<Value>,
    pub(crate) idempotency_key: Option<&'a str>,
}

pub(crate) fn record_response(
    world: &SharedWorld,
    status: u16,
    trace_id: Option<String>,
    body: Value,
) {
    let mut ctx = world.borrow_mut();
    ctx.last_status = Some(status);
    ctx.last_trace_id = trace_id;
    ctx.last_body = Some(body);
}

pub(crate) fn session_cookie(world: &SharedWorld) -> String {
    world
        .borrow()
        .session_cookie
        .clone()
        .expect("session cookie")
        .split(';')
        .next()
        .expect("cookie pair")
        .to_owned()
}

pub(crate) fn login_and_store_cookie(world: &SharedWorld) {
    let (status, cookie_header) = with_world_async(world, |base_url| async move {
        let response = Client::default()
            .post(format!("{base_url}/api/v1/login"))
            .send_json(&serde_json::json!({
                "username": "admin",
                "password": "password"
            }))
            .await
            .expect("login request");

        let status = response.status().as_u16();
        let cookie_header = response
            .headers()
            .get(header::SET_COOKIE)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_owned());
        (status, cookie_header)
    });

    let mut ctx = world.borrow_mut();
    ctx.last_status = Some(status);
    ctx.session_cookie = cookie_header;
    ctx.last_trace_id = None;
    ctx.last_body = None;
}

pub(crate) fn perform_json_request(world: &SharedWorld, spec: JsonRequest<'_>) {
    let cookie = spec.include_cookie.then(|| session_cookie(world));
    let (status, trace_id, body) = with_world_async(world, |base_url| async move {
        let mut request =
            Client::default().request(spec.method, format!("{base_url}{}", spec.path));
        if let Some(cookie) = cookie {
            request = request.insert_header((header::COOKIE, cookie));
        }
        if let Some(key) = spec.idempotency_key {
            request = request.insert_header((IDEMPOTENCY_KEY_HEADER, key));
        }
        let mut response = match spec.payload {
            Some(payload) => request.send_json(&payload).await.expect("json request"),
            None => request.send().await.expect("request"),
        };
        let status = response.status().as_u16();
        let trace_id = response
            .headers()
            .get(TRACE_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_owned());
        let body = response.body().await.expect("body");
        let json: Value = serde_json::from_slice(&body).expect("json body");
        (status, trace_id, json)
    });

    record_response(world, status, trace_id, body);
}
