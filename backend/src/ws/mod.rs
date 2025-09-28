//! WebSocket entry and routing.

use actix_web::web::Payload;
use actix_web::{get, http::header::ORIGIN, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use tracing::error;

pub mod display_name;
pub mod messages;
pub mod socket;

/// Handle WebSocket upgrade for the `/ws` endpoint.
#[get("/ws")]
pub async fn ws_entry(req: HttpRequest, stream: Payload) -> actix_web::Result<HttpResponse> {
    if let Some(origin) = req.headers().get(ORIGIN) {
        let origin = origin.to_str().unwrap_or_default();
        let allowed = origin.starts_with("http://localhost:")
            || origin.ends_with(".yourdomain.example")
            || origin == "https://yourdomain.example";
        if !allowed {
            error!(%origin, "Rejected WS upgrade due to disallowed Origin");
            // TODO: Externalise the origin allow-list via configuration once available.
            return Err(actix_web::error::ErrorForbidden("Origin not allowed"));
        }
    }

    let actor = socket::UserSocket::default();
    ws::start(actor, &req, stream).map_err(|e| {
        error!(error = %e, "WebSocket upgrade failed");
        actix_web::error::ErrorInternalServerError("WebSocket upgrade failed")
    })
}
