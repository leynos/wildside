use actix::{Actor, StreamHandler};
use actix_web_actors::ws::{self, Message, ProtocolError};

#[derive(Default)]
pub struct UserSocket;

impl Actor for UserSocket {
    type Context = ws::WebsocketContext<Self>;
}

impl StreamHandler<Result<Message, ProtocolError>> for UserSocket {
    fn handle(&mut self, _: Result<Message, ProtocolError>, _: &mut Self::Context) {
        // For now, ignore all incoming messages.
    }
}
