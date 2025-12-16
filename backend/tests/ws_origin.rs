//! Behavioural tests for WebSocket origin validation.

#[path = "support/ws.rs"]
mod ws_support;

use actix_http::Request;
use actix_web::http::header::HeaderValue;
use actix_web::{
    body::BoxBody,
    dev::{Service, ServiceResponse},
    http::{header, StatusCode},
    test::{self, TestRequest},
    web, App,
};
use backend::domain::UserOnboardingService;
use backend::inbound::ws;
use backend::inbound::ws::state::WsState;
use rstest::{fixture, rstest};

// Example Sec-WebSocket-Key from RFC 6455 section 1.3 used to satisfy handshake requirements.
const RFC6455_SAMPLE_KEY: &str = "dGhlIHNhbXBsZSBub25jZQ==";

#[fixture]
fn ws_state() -> WsState {
    ws_support::ws_state(UserOnboardingService)
}

async fn init_app(
    state: WsState,
) -> impl Service<Request, Response = ServiceResponse<BoxBody>, Error = actix_web::Error> {
    test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .service(ws::ws_entry),
    )
    .await
}

fn handshake_request() -> TestRequest {
    TestRequest::get()
        .uri("/ws")
        .insert_header((header::UPGRADE, "websocket"))
        .insert_header((header::CONNECTION, "Upgrade"))
        .insert_header((header::SEC_WEBSOCKET_VERSION, "13"))
        .insert_header((header::SEC_WEBSOCKET_KEY, RFC6455_SAMPLE_KEY))
}

#[derive(Debug, Clone, Copy)]
/// Test cases for Origin header validation.
///
/// Covers both missing/invalid headers and disallowed origin values.
enum OriginHeaderCase {
    /// No Origin header present.
    Missing,
    /// Origin not in the allowlist.
    Unlisted,
    /// Multiple Origin headers (forbidden by RFC 6455).
    Multiple,
    /// Malformed Origin header (invalid UTF-8).
    Malformed,
    /// Localhost with port 0 (not a valid listening port).
    LocalhostZeroPort,
}

fn handshake_request_for_origin_case(origin_case: OriginHeaderCase) -> Request {
    match origin_case {
        OriginHeaderCase::Missing => handshake_request().to_request(),
        OriginHeaderCase::Unlisted => handshake_request()
            .append_header((header::ORIGIN, "https://example.com"))
            .to_request(),
        OriginHeaderCase::Multiple => handshake_request()
            .append_header((header::ORIGIN, "https://yourdomain.example"))
            .append_header((header::ORIGIN, "https://example.com"))
            .to_request(),
        OriginHeaderCase::Malformed => {
            // Byte 0x80 is invalid UTF-8, tests parser robustness.
            let invalid = HeaderValue::from_bytes(&[0x80]).expect("opaque Origin header value");
            handshake_request()
                .insert_header((header::ORIGIN, invalid))
                .to_request()
        }
        OriginHeaderCase::LocalhostZeroPort => handshake_request()
            .insert_header((header::ORIGIN, "http://localhost:0"))
            .to_request(),
    }
}

#[rstest]
#[case("https://yourdomain.example")]
#[case("https://chat.yourdomain.example")]
#[case("http://localhost:3000")]
fn upgrades_when_origin_allowed(ws_state: WsState, #[case] origin: &str) {
    actix_rt::System::new().block_on(async move {
        let app = init_app(ws_state).await;

        let req = handshake_request()
            .insert_header((header::ORIGIN, origin))
            .to_request();
        let response = test::call_service(&app, req).await;
        assert_eq!(
            response.status(),
            StatusCode::SWITCHING_PROTOCOLS,
            "origin {origin}"
        );
    });
}

#[rstest]
#[case(OriginHeaderCase::Missing, StatusCode::FORBIDDEN)]
#[case(OriginHeaderCase::Unlisted, StatusCode::FORBIDDEN)]
#[case(OriginHeaderCase::Multiple, StatusCode::BAD_REQUEST)]
#[case(OriginHeaderCase::Malformed, StatusCode::BAD_REQUEST)]
#[case(OriginHeaderCase::LocalhostZeroPort, StatusCode::FORBIDDEN)]
fn rejects_disallowed_origin_headers(
    ws_state: WsState,
    #[case] origin_case: OriginHeaderCase,
    #[case] expected: StatusCode,
) {
    actix_rt::System::new().block_on(async move {
        let app = init_app(ws_state).await;

        let req = handshake_request_for_origin_case(origin_case);
        let response = test::call_service(&app, req).await;
        assert_eq!(response.status(), expected, "{origin_case:?}");
    });
}
