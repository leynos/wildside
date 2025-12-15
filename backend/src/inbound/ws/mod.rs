//! WebSocket inbound adapter bridging domain events to client payloads.
//!
//! Responsibilities:
//! - validate upgrade requests (origin allow-list)
//! - initialise the per-connection WebSocket actor
//! - keep WebSocket-specific concerns at the edge of the system

use actix_web::web::{self, Payload};
use actix_web::{
    get,
    http::header::{HeaderValue, ORIGIN},
    HttpRequest, HttpResponse,
};
use actix_web_actors::ws;
use tracing::{error, warn};
use url::Url;

mod session;

pub mod messages;
pub mod state;

/// Handle WebSocket upgrade for the `/ws` endpoint.
#[get("/ws")]
pub async fn ws_entry(
    state: web::Data<state::WsState>,
    req: HttpRequest,
    stream: Payload,
) -> actix_web::Result<HttpResponse> {
    let mut origin_iter = req.headers().get_all(ORIGIN);
    let origin_header = origin_iter.next().ok_or_else(|| {
        error!("Missing Origin header on WebSocket upgrade");
        actix_web::error::ErrorForbidden("Origin not allowed")
    })?;
    if origin_iter.next().is_some() {
        error!("Multiple Origin headers on WebSocket upgrade");
        return Err(actix_web::error::ErrorBadRequest("Invalid Origin header"));
    }

    validate_origin(origin_header)?;

    let actor = session::WsSession::new(state.onboarding.clone());
    ws::start(actor, &req, stream).map_err(|error| {
        error!(error = %error, "WebSocket upgrade failed");
        actix_web::error::ErrorInternalServerError("WebSocket upgrade failed")
    })
}

fn validate_origin(origin_header: &HeaderValue) -> actix_web::Result<()> {
    let origin_value = match origin_header.to_str() {
        Ok(value) => value,
        Err(error) => {
            error!(error = %error, "Failed to parse Origin header as string");
            return Err(actix_web::error::ErrorBadRequest("Invalid Origin header"));
        }
    };

    let origin = Url::parse(origin_value).map_err(|error| {
        error!(error = %error, "Failed to parse Origin header as URL");
        actix_web::error::ErrorBadRequest("Invalid Origin header")
    })?;

    if is_allowed_origin(&origin) {
        Ok(())
    } else {
        warn!(
            origin = origin_value,
            "Rejected WS upgrade due to disallowed Origin"
        );
        Err(actix_web::error::ErrorForbidden("Origin not allowed"))
    }
}

const PRIMARY_HOST: &str = "yourdomain.example";
const LOCALHOST: &str = "localhost";
const ALLOWED_SUBDOMAIN_SUFFIX: &str = ".yourdomain.example";

/// Returns true when a parsed Origin belongs to the static allow-list.
///
/// The allow-list currently accepts HTTPS requests from the production root
/// domain and any of its subdomains, and HTTP requests from localhost with a
/// non-zero explicit port. Once configuration is available this should move
/// into a runtime-controlled allow-list.
fn is_allowed_origin(origin: &Url) -> bool {
    let host = match origin.host_str() {
        Some(value) => value,
        None => return false,
    };

    match origin.scheme() {
        "http" if host == LOCALHOST => matches!(origin.port(), Some(port) if port != 0),
        "https" if host == PRIMARY_HOST => true,
        "https" if host.strip_suffix(ALLOWED_SUBDOMAIN_SUFFIX).is_some() => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http::{header::HeaderValue, StatusCode};
    use rstest::rstest;

    fn header(value: &str) -> HeaderValue {
        HeaderValue::from_str(value).expect("valid header value")
    }

    #[rstest]
    #[case("http://localhost:3000")]
    #[case("https://yourdomain.example")]
    #[case("https://chat.yourdomain.example")]
    fn accepts_configured_origins(#[case] origin: &str) {
        let header = header(origin);
        assert!(validate_origin(&header).is_ok());
    }

    #[rstest]
    #[case("http://localhost")]
    #[case("https://example.com")]
    #[case("wss://yourdomain.example")]
    fn rejects_disallowed_origins(#[case] origin: &str) {
        let header = header(origin);
        let error = validate_origin(&header).expect_err("origin should be rejected");
        assert_eq!(
            error.as_response_error().status_code(),
            StatusCode::FORBIDDEN
        );
    }

    #[test]
    fn rejects_non_utf8_origin_header() {
        let header = HeaderValue::from_bytes(&[0x80]).expect("opaque header value");
        let error = validate_origin(&header).expect_err("origin should be rejected");
        assert_eq!(
            error.as_response_error().status_code(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn rejects_unparsable_origin_header() {
        let header = HeaderValue::from_static("not a url");
        let error = validate_origin(&header).expect_err("origin should be rejected");
        assert_eq!(
            error.as_response_error().status_code(),
            StatusCode::BAD_REQUEST
        );
    }

    #[rstest]
    #[case("http://localhost:4000", true)]
    #[case("http://localhost:0", false)]
    #[case("http://localhost", false)]
    #[case("https://yourdomain.example", true)]
    #[case("https://chat.yourdomain.example", true)]
    #[case("https://yourdomain.example.evil.com", false)]
    #[case("wss://yourdomain.example", false)]
    fn evaluates_allow_list(#[case] origin: &str, #[case] expected: bool) {
        let parsed = Url::parse(origin).expect("url should parse");
        assert_eq!(is_allowed_origin(&parsed), expected);
    }
}
