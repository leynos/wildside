//! WebSocket actor for user connections.

use std::time::{Duration, Instant};

use actix::{Actor, ActorContext, AsyncContext, StreamHandler};
use actix_web_actors::ws::{self, Message, ProtocolError};
use tracing::warn;

/// Time between heartbeats to the client.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// Maximum allowed time between messages from the client before considering it disconnected.
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

pub struct UserSocket {
    last_heartbeat: Instant,
}

impl Default for UserSocket {
    fn default() -> Self {
        Self {
            last_heartbeat: Instant::now(),
        }
    }
}

impl Actor for UserSocket {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.last_heartbeat = Instant::now();
        ctx.run_interval(HEARTBEAT_INTERVAL, |actor, ctx| {
            if Instant::now().duration_since(actor.last_heartbeat) > CLIENT_TIMEOUT {
                ctx.stop();
                return;
            }
            ctx.ping(b"");
        });
    }
}

impl StreamHandler<Result<Message, ProtocolError>> for UserSocket {
    fn handle(&mut self, msg: Result<Message, ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(Message::Ping(payload)) => {
                self.last_heartbeat = Instant::now();
                ctx.pong(&payload);
            }
            Ok(Message::Pong(_)) | Ok(Message::Text(_)) | Ok(Message::Binary(_)) => {
                self.last_heartbeat = Instant::now();
            }
            Ok(Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            Ok(Message::Nop) => {}
            Ok(Message::Continuation(_)) => {}
            Err(err) => {
                warn!(error = %err, "WebSocket protocol error");
                ctx.stop();
            }
        }
    }
}
