//! Per-connection WebSocket actor.
//!
//! Keeps WebSocket framing and heartbeats at the edge while deferring
//! application behaviour to the domain (`UserOnboardingService`).

use std::time::{Duration, Instant};

use crate::domain::{DisplayNameSubmission, UserEvent, UserOnboardingService};
use crate::inbound::ws::messages::{
    DisplayNameRequest, Envelope, InvalidDisplayNamePayload, UserCreatedPayload,
};
use actix::{Actor, ActorContext, AsyncContext, StreamHandler};
use actix_web_actors::ws::{self, CloseCode, CloseReason, Message, ProtocolError};
use tracing::warn;

/// Time between heartbeats to the client.
#[cfg(not(test))]
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
#[cfg(test)]
const HEARTBEAT_INTERVAL: Duration = Duration::from_millis(50);

/// Maximum allowed time between messages from the client before considering it disconnected.
#[cfg(not(test))]
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(test)]
const CLIENT_TIMEOUT: Duration = Duration::from_millis(100);

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
        &mut self,
        request: DisplayNameRequest,
        ctx: &mut ws::WebsocketContext<Self>,
    ) {
        let submission = DisplayNameSubmission::from(request);
        let event = self.onboarding.register(submission);
        self.handle_user_event(event, ctx);
    }

    fn send_json<T: serde::Serialize>(&self, ctx: &mut ws::WebsocketContext<Self>, payload: &T) {
        match serde_json::to_string(payload) {
            Ok(body) => ctx.text(body),
            Err(err) => warn!(error = %err, "Failed to serialize WebSocket payload"),
        }
    }

    fn close_with_policy_error(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.close(Some(CloseReason {
            code: CloseCode::Policy,
            description: Some("invalid payload".into()),
        }));
        ctx.stop();
    }

    fn handle_user_event(&mut self, event: UserEvent, ctx: &mut ws::WebsocketContext<Self>) {
        match event {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inbound::ws;
    use actix_web::{dev::Server, http::header, App, HttpServer};
    use awc::ws::{Frame, Message};
    use futures_util::{SinkExt, StreamExt};
    use serde_json::Value;
    use uuid::Uuid;

    async fn start_ws_server() -> (String, Server) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let addr = listener.local_addr().expect("listener addr");
        let server = HttpServer::new(|| App::new().service(ws::ws_entry))
            .listen(listener)
            .expect("bind test server")
            .disable_signals()
            .run();
        let url = format!("http://{addr}");
        (url, server)
    }

    fn handshake_request_payload(name: &str) -> String {
        serde_json::json!({
            "traceId": Uuid::nil(),
            "displayName": name
        })
        .to_string()
    }

    #[actix_rt::test]
    async fn sends_user_created_event_for_valid_payload() {
        let (url, server) = start_ws_server().await;
        actix_web::rt::spawn(server);

        let (_resp, mut socket) = awc::Client::default()
            .ws(format!("{url}/ws"))
            .set_header(header::ORIGIN, "http://localhost:3000")
            .connect()
            .await
            .expect("websocket connect");

        socket
            .send(Message::Text(handshake_request_payload("Bob").into()))
            .await
            .expect("send text");

        let frame = socket.next().await.expect("response frame").expect("frame");
        let text = match frame {
            Frame::Text(bytes) => bytes,
            other => panic!("expected text frame, got {other:?}"),
        };
        let value: Value = serde_json::from_slice(&text).expect("json");
        assert_eq!(
            value.get("displayName").and_then(Value::as_str),
            Some("Bob")
        );
        assert!(value.get("id").is_some(), "user id present");
        assert_eq!(
            value.get("traceId").and_then(Value::as_str),
            Some(Uuid::nil().to_string().as_str())
        );
    }

    #[actix_rt::test]
    async fn sends_rejection_for_invalid_payload() {
        let (url, server) = start_ws_server().await;
        actix_web::rt::spawn(server);
        let (_resp, mut socket) = awc::Client::default()
            .ws(format!("{url}/ws"))
            .set_header(header::ORIGIN, "http://localhost:3000")
            .connect()
            .await
            .expect("websocket connect");

        socket
            .send(Message::Text(handshake_request_payload("bad$char").into()))
            .await
            .expect("send text");

        let frame = socket.next().await.expect("response frame").expect("frame");
        let text = match frame {
            Frame::Text(bytes) => bytes,
            other => panic!("expected text frame, got {other:?}"),
        };
        let value: Value = serde_json::from_slice(&text).expect("json");
        assert_eq!(
            value.get("code").and_then(Value::as_str),
            Some("invalid_chars")
        );
        assert_eq!(
            value
                .get("details")
                .and_then(|v| v.get("field"))
                .and_then(Value::as_str),
            Some("displayName")
        );
    }

    #[actix_rt::test]
    async fn closes_on_malformed_json() {
        let (url, server) = start_ws_server().await;
        actix_web::rt::spawn(server);
        let (_resp, mut socket) = awc::Client::default()
            .ws(format!("{url}/ws"))
            .set_header(header::ORIGIN, "http://localhost:3000")
            .connect()
            .await
            .expect("websocket connect");

        socket
            .send(awc::ws::Message::Text("not-json".into()))
            .await
            .expect("send text");

        let frame = socket.next().await.expect("response frame").expect("frame");
        match frame {
            Frame::Close(reason) => {
                assert_eq!(reason.expect("reason").code, CloseCode::Policy);
            }
            other => panic!("expected close frame, got {other:?}"),
        }
    }

    #[actix_rt::test]
    async fn closes_after_timeout_without_client_messages() {
        let (url, server) = start_ws_server().await;
        actix_web::rt::spawn(server);
        let (_resp, mut socket) = awc::Client::default()
            .ws(format!("{url}/ws"))
            .set_header(header::ORIGIN, "http://localhost:3000")
            .connect()
            .await
            .expect("websocket connect");

        tokio::time::sleep(CLIENT_TIMEOUT + HEARTBEAT_INTERVAL * 3).await;

        let mut observed_close = None;
        while let Some(frame) = socket.next().await {
            let frame = frame.expect("frame");
            match frame {
                Frame::Ping(_) | Frame::Pong(_) => continue,
                Frame::Close(reason) => {
                    observed_close = reason;
                    break;
                }
                other => panic!("unexpected frame before close: {other:?}"),
            }
        }

        let reason = observed_close.expect("close frame missing after timeout");
        assert_eq!(reason.code, CloseCode::Normal);
        assert_eq!(reason.description.as_deref(), Some("heartbeat timeout"));
    }
}
