//! WebSocket entry and routing.

use actix_web::web::Payload;
use actix_web::{get, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use tracing::error;

pub mod messages;
pub mod socket;

/// Handle WebSocket upgrade for the `/ws` endpoint.
#[get("/ws")]
pub async fn ws_entry(req: HttpRequest, stream: Payload) -> actix_web::Result<HttpResponse> {
    let actor = socket::UserSocket::default();
    ws::start(actor, &req, stream).map_err(|e| {
        error!(error = %e, "WebSocket upgrade failed");
        actix_web::error::ErrorInternalServerError("WebSocket upgrade failed")
    })
}
