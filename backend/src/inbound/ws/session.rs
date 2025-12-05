//! Per-connection WebSocket actor.
//!
//! Keeps WebSocket framing and heartbeats at the edge while deferring
//! application behaviour to the domain (`UserOnboardingService`).

use std::time::{Duration, Instant};

use crate::domain::{DisplayNameSubmission, UserEvent, UserOnboardingService};
use crate::inbound::ws::messages::{
    DisplayNameRequest, Envelope, InvalidDisplayNamePayload, UserCreatedPayload, UserEventMessage,
};
use actix::{Actor, ActorContext, AsyncContext, Handler, StreamHandler};
use actix_web_actors::ws::{self, CloseCode, CloseReason, Message, ProtocolError};
use tracing::warn;

/// Time between heartbeats to the client.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// Maximum allowed time between messages from the client before considering it disconnected.
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

pub struct WsSession {
    last_heartbeat: Instant,
    onboarding: UserOnboardingService,
}

impl Default for WsSession {
    fn default() -> Self {
        Self {
            last_heartbeat: Instant::now(),
            onboarding: UserOnboardingService,
        }
    }
}

impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.last_heartbeat = Instant::now();
        ctx.run_interval(HEARTBEAT_INTERVAL, |actor, ctx| {
            if Instant::now().duration_since(actor.last_heartbeat) > CLIENT_TIMEOUT {
                warn!("WebSocket heartbeat timeout; closing connection");
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

impl WsSession {
    fn handle_display_name_request(
        &self,
        request: DisplayNameRequest,
        ctx: &mut ws::WebsocketContext<Self>,
    ) {
        let submission = DisplayNameSubmission::from(request);
        let event = self.onboarding.register(submission);
        ctx.address().do_send(UserEventMessage(event));
    }

    fn send_json<T: serde::Serialize>(&self, ctx: &mut ws::WebsocketContext<Self>, payload: &T) {
        match serde_json::to_string(payload) {
            Ok(body) => ctx.text(body),
            Err(err) => warn!(error = %err, "Failed to serialise WebSocket payload"),
        }
    }

    fn close_with_policy_error(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.close(Some(CloseReason {
            code: CloseCode::Policy,
            description: Some("invalid payload".into()),
        }));
        ctx.stop();
    }
}

impl StreamHandler<Result<Message, ProtocolError>> for WsSession {
    fn handle(&mut self, msg: Result<Message, ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(Message::Ping(payload)) => {
                self.last_heartbeat = Instant::now();
                ctx.pong(&payload);
            }
            Ok(Message::Text(text)) => {
                self.last_heartbeat = Instant::now();
                match serde_json::from_str::<DisplayNameRequest>(&text) {
                    Ok(request) => self.handle_display_name_request(request, ctx),
                    Err(error) => {
                        warn!(error = %error, "Rejected malformed WebSocket payload");
                        self.close_with_policy_error(ctx);
                    }
                }
            }
            Ok(Message::Pong(_)) | Ok(Message::Binary(_)) => {
                self.last_heartbeat = Instant::now();
            }
            Ok(Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            Ok(Message::Nop) | Ok(Message::Continuation(_)) => {}
            Err(err) => {
                warn!(error = %err, "WebSocket protocol error");
                ctx.stop();
            }
        }
    }
}

impl Handler<UserEventMessage> for WsSession {
    type Result = ();

    fn handle(&mut self, msg: UserEventMessage, ctx: &mut Self::Context) {
        match msg.0 {
            UserEvent::UserCreated(event) => {
                let envelope: Envelope<UserCreatedPayload> = event.into();
                self.send_json(ctx, &envelope);
            }
            UserEvent::DisplayNameRejected(event) => {
                let envelope: Envelope<InvalidDisplayNamePayload> = event.into();
                self.send_json(ctx, &envelope);
            }
        }
    }
}
