//! WebSocket actor for user connections.

use std::time::{Duration, Instant};

use crate::ws::messages::UserCreated;
use actix::{Actor, ActorContext, AsyncContext, Handler, StreamHandler};
use actix_web_actors::ws::{self, CloseCode, CloseReason, Message, ProtocolError};
use regex::Regex;
use tracing::{info, warn};
use uuid::Uuid;

/// Time between heartbeats to the client.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// Maximum allowed time between messages from the client before considering it disconnected.
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

fn validate_display_name(name: &str) -> bool {
    // Only allow alphanumeric, spaces, and underscores. Length 3–32.
    let re = Regex::new(r"^[A-Za-z0-9_ ]{3,32}$").unwrap();
    re.is_match(name)
}

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
                info!("WebSocket heartbeat timeout; closing connection");
                ctx.close(Some(CloseReason {
                    code: CloseCode::Normal,
                    description: Some("heartbeat timeout".into()),
                }));
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
            Ok(Message::Text(name)) => {
                self.last_heartbeat = Instant::now();
                if validate_display_name(&name) {
                    let event = UserCreated::new(Uuid::new_v4().to_string(), name.to_string());
                    if let Ok(body) = serde_json::to_string(&event) {
                        ctx.text(body);
                    }
                } else {
                    let error_msg = serde_json::json!({
                        "error": "Invalid display name. Only alphanumeric characters, spaces, and underscores are allowed. Length must be between 3 and 32 characters."
                    });
                    ctx.text(error_msg.to_string());
                }
            }
            Ok(Message::Pong(_)) | Ok(Message::Binary(_)) => {
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

impl Handler<UserCreated> for UserSocket {
    type Result = ();

    fn handle(&mut self, msg: UserCreated, ctx: &mut Self::Context) {
        match serde_json::to_string(&msg) {
            Ok(body) => ctx.text(body),
            Err(err) => warn!(error = %err, "Failed to serialise UserCreated event"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::validate_display_name;
    use rstest::rstest;

    #[rstest]
    #[case(String::from("ab"), false)]
    #[case("a".repeat(33), false)]
    #[case(String::from("Alice_Bob 123"), true)]
    #[case(String::from("bad$char"), false)]
    fn validate_display_name_cases(#[case] name: String, #[case] expected: bool) {
        assert_eq!(validate_display_name(&name), expected);
    }
}
