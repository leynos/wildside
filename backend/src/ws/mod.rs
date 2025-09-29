//! WebSocket entry and routing.

use actix_web::web::Payload;
use actix_web::{
    get,
    http::header::{HeaderValue, ORIGIN},
    HttpRequest, HttpResponse,
};
use actix_web_actors::ws;
use tracing::error;
use url::Url;

pub mod display_name;
pub mod messages;
pub mod socket;

/// Handle WebSocket upgrade for the `/ws` endpoint.
#[get("/ws")]
pub async fn ws_entry(req: HttpRequest, stream: Payload) -> actix_web::Result<HttpResponse> {
    let origin_header = req.headers().get(ORIGIN).ok_or_else(|| {
        error!("Missing Origin header on WebSocket upgrade");
        actix_web::error::ErrorForbidden("Origin not allowed")
    })?;

    validate_origin(origin_header)?;

    let actor = socket::UserSocket::default();
    ws::start(actor, &req, stream).map_err(|error| {
        error!(error = %error, "WebSocket upgrade failed");
        actix_web::error::ErrorInternalServerError("WebSocket upgrade failed")
    })
}

fn validate_origin(origin_header: &HeaderValue) -> actix_web::Result<()> {
    let origin_value = origin_header.to_str().map_err(|error| {
        error!(error = %error, "Failed to parse Origin header as string");
        actix_web::error::ErrorBadRequest("Invalid Origin header")
    })?;

    let origin = Url::parse(origin_value).map_err(|error| {
        error!(error = %error, "Failed to parse Origin header as URL");
        actix_web::error::ErrorBadRequest("Invalid Origin header")
    })?;

    if is_allowed_origin(&origin) {
        Ok(())
    } else {
        error!(
            origin = origin_value,
            "Rejected WS upgrade due to disallowed Origin"
        );
        Err(actix_web::error::ErrorForbidden("Origin not allowed"))
    }
}

/// TODO: Externalise the origin allow-list via configuration once available.
fn is_allowed_origin(origin: &Url) -> bool {
    match origin.scheme() {
        "http" | "https" => {}
        _ => return false,
    }

    let Some(host) = origin.host_str() else {
        return false;
    };

    if origin.scheme() == "http" && host == "localhost" {
        return origin.port().is_some();
    }

    if origin.scheme() == "https" && host == "yourdomain.example" {
        return true;
    }

    host.ends_with(".yourdomain.example")
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
