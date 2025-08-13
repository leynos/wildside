use actix_web::web::Payload;
use actix_web::{get, HttpRequest, HttpResponse};
use actix_web_actors::ws;

pub mod socket; // your ws::start actor lives here

#[get("/ws")]
pub async fn ws_entry(req: HttpRequest, stream: Payload) -> HttpResponse {
    let actor = socket::UserSocket;
    match ws::start(actor, &req, stream) {
        Ok(resp) => resp,
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}
