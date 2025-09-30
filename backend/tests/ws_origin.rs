//! Behavioural tests for WebSocket origin validation.

use actix_web::http::header::HeaderValue;
use actix_web::{
    http::{header, StatusCode},
    test::{self, TestRequest},
    App,
};
use backend::ws;

// Example Sec-WebSocket-Key from RFC 6455 section 1.3 used to satisfy handshake requirements.
const RFC6455_SAMPLE_KEY: &str = "dGhlIHNhbXBsZSBub25jZQ==";

fn handshake_request() -> TestRequest {
    TestRequest::get()
        .uri("/ws")
        .insert_header((header::UPGRADE, "websocket"))
        .insert_header((header::CONNECTION, "Upgrade"))
        .insert_header((header::SEC_WEBSOCKET_VERSION, "13"))
        .insert_header((header::SEC_WEBSOCKET_KEY, RFC6455_SAMPLE_KEY))
}

#[actix_rt::test]
async fn upgrades_when_origin_allowed() {
    let app = test::init_service(App::new().service(ws::ws_entry)).await;

    for origin in [
        "https://yourdomain.example",
        "https://chat.yourdomain.example",
        "http://localhost:3000",
    ] {
        let req = handshake_request()
            .insert_header((header::ORIGIN, origin))
            .to_request();
        let response = test::call_service(&app, req).await;
        assert_eq!(
            response.status(),
            StatusCode::SWITCHING_PROTOCOLS,
            "origin {origin}"
        );
    }
}

#[actix_rt::test]
async fn rejects_missing_origin_header() {
    let app = test::init_service(App::new().service(ws::ws_entry)).await;

    let req = handshake_request().to_request();
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[actix_rt::test]
async fn rejects_unlisted_origin() {
    let app = test::init_service(App::new().service(ws::ws_entry)).await;

    let req = handshake_request()
        .append_header((header::ORIGIN, "https://example.com"))
        .to_request();
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[actix_rt::test]
async fn rejects_multiple_origin_headers() {
    let app = test::init_service(App::new().service(ws::ws_entry)).await;

    let req = handshake_request()
        .append_header((header::ORIGIN, "https://yourdomain.example"))
        .append_header((header::ORIGIN, "https://example.com"))
        .to_request();
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[actix_rt::test]
async fn rejects_malformed_origin_header() {
    let app = test::init_service(App::new().service(ws::ws_entry)).await;

    let invalid = HeaderValue::from_bytes(&[0x80]).expect("opaque Origin header value");
    let req = handshake_request()
        .insert_header((header::ORIGIN, invalid))
        .to_request();
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
