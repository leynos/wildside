//! Per-connection WebSocket actor.
//!
//! Keeps WebSocket framing and heartbeats at the edge while deferring
//! application behaviour to the domain (`UserOnboardingService`). The public
//! WebSocket contract pings every 5s and considers a connection idle after
//! 10s without client traffic. Tests shorten these intervals to speed up
//! feedback; adjust the constants below if SLAs change so clients and
//! intermediaries stay aligned.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::domain::ports::UserOnboarding;
use crate::domain::{UserEvent, UserOnboardingService};
use crate::inbound::ws::messages::{
    DisplayNameRequest, InvalidDisplayNameResponse, UserCreatedResponse,
};
use crate::middleware::trace::TraceId;
use actix::{Actor, ActorContext, AsyncContext, StreamHandler};
use actix_web_actors::ws::{self, CloseCode, CloseReason, Message, ProtocolError};
use tracing::warn;

/// Time between heartbeats to the client (5s in production, shorter in tests).
#[cfg(not(test))]
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
#[cfg(test)]
const HEARTBEAT_INTERVAL: Duration = Duration::from_millis(50);

/// Max idle time before disconnecting the client (10s in production, shorter in tests).
#[cfg(not(test))]
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(test)]
const CLIENT_TIMEOUT: Duration = Duration::from_millis(100);

pub struct WsSession {
    last_heartbeat: Instant,
    onboarding: Arc<dyn UserOnboarding>,
}

impl Default for WsSession {
    fn default() -> Self {
        Self::new(Arc::new(UserOnboardingService))
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
    pub fn new(onboarding: Arc<dyn UserOnboarding>) -> Self {
        Self {
            last_heartbeat: Instant::now(),
            onboarding,
        }
    }

    fn handle_display_name_request(
        &mut self,
        request: DisplayNameRequest,
        ctx: &mut ws::WebsocketContext<Self>,
    ) {
        let trace_id = TraceId::from_uuid(request.trace_id);
        // `register` must remain CPU-bound; any I/O work should be offloaded to other actors/tasks.
        let event = self.onboarding.register(trace_id, request.display_name);
        self.handle_user_event(event, ctx);
    }

    fn send_json<T: serde::Serialize>(&self, ctx: &mut ws::WebsocketContext<Self>, payload: &T) {
        match serde_json::to_string(payload) {
            Ok(body) => ctx.text(body),
            Err(err) => {
                // In debug builds fail fast so schema drift is fixed; in release we log and keep the connection alive.
                if cfg!(debug_assertions) {
                    panic!("domain events must serialize: {err}");
                } else {
                    warn!(error = %err, "Failed to serialize WebSocket payload");
                }
            }
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
                let response: UserCreatedResponse = event.into();
                self.send_json(ctx, &response);
            }
            UserEvent::DisplayNameRejected(event) => {
                let response: InvalidDisplayNameResponse = event.into();
                self.send_json(ctx, &response);
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
            Ok(Message::Nop) | Ok(Message::Continuation(_)) => {
                self.last_heartbeat = Instant::now();
            }
            Err(err) => {
                warn!(error = %err, "WebSocket protocol error");
                ctx.close(Some(CloseReason {
                    code: CloseCode::Protocol,
                    description: Some("protocol error".into()),
                }));
                ctx.stop();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inbound::ws;
    use crate::inbound::ws::state::WsState;
    use actix_web::{dev::Server, dev::ServerHandle, http::header, App, HttpServer};
    use awc::{ws::Codec, ws::Frame, ws::Message, BoxedSocket};
    use futures_util::{SinkExt, StreamExt};
    use rstest::{fixture, rstest};
    use serde_json::Value;
    use std::sync::Arc;
    use uuid::Uuid;

    #[fixture]
    async fn start_ws_server() -> (String, Server) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let addr = listener.local_addr().expect("listener addr");
        let ws_state = WsState::new(Arc::new(UserOnboardingService));
        let server = HttpServer::new(move || {
            App::new()
                .app_data(actix_web::web::Data::new(ws_state.clone()))
                .service(ws::ws_entry)
        })
        .listen(listener)
        .expect("bind test server")
        .disable_signals()
        .run();
        let url = format!("http://{addr}");
        (url, server)
    }

    #[fixture]
    async fn ws_client(
        #[future] start_ws_server: (String, Server),
    ) -> (actix_codec::Framed<BoxedSocket, Codec>, ServerHandle) {
        let (url, server) = start_ws_server.await;
        let handle = server.handle();
        actix_web::rt::spawn(server);

        let (_resp, socket) = awc::Client::default()
            .ws(format!("{url}/ws"))
            .set_header(header::ORIGIN, "http://localhost:3000")
            .connect()
            .await
            .expect("websocket connect");

        (socket, handle)
    }

    fn handshake_request_payload(name: &str) -> String {
        serde_json::json!({
            "traceId": Uuid::nil(),
            "displayName": name
        })
        .to_string()
    }

    #[rstest]
    #[actix_rt::test]
    async fn sends_user_created_event_for_valid_payload(
        #[future] ws_client: (actix_codec::Framed<BoxedSocket, Codec>, ServerHandle),
    ) {
        let (mut socket, _server): (actix_codec::Framed<_, _>, _) = ws_client.await;
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

    #[rstest]
    #[actix_rt::test]
    async fn sends_rejection_for_invalid_payload(
        #[future] ws_client: (actix_codec::Framed<BoxedSocket, Codec>, ServerHandle),
    ) {
        let (mut socket, _server): (actix_codec::Framed<_, _>, _) = ws_client.await;
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

    #[rstest]
    #[actix_rt::test]
    async fn closes_on_malformed_json(
        #[future] ws_client: (actix_codec::Framed<BoxedSocket, Codec>, ServerHandle),
    ) {
        let (mut socket, _server): (actix_codec::Framed<_, _>, _) = ws_client.await;
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

    #[rstest]
    #[actix_rt::test]
    async fn closes_after_timeout_without_client_messages(
        #[future] ws_client: (actix_codec::Framed<BoxedSocket, Codec>, ServerHandle),
    ) {
        let (mut socket, _server): (actix_codec::Framed<_, _>, _) = ws_client.await;
        tokio::time::sleep(CLIENT_TIMEOUT + HEARTBEAT_INTERVAL * 3).await;

        use std::time::Duration;

        let observed_close = tokio::time::timeout(Duration::from_secs(2), async {
            let mut observed = None;
            while let Some(frame) = socket.next().await {
                let frame = frame.expect("frame");
                match frame {
                    Frame::Ping(_) | Frame::Pong(_) => continue,
                    Frame::Close(reason) => {
                        observed = reason;
                        break;
                    }
                    other => panic!("unexpected frame before close: {other:?}"),
                }
            }
            observed
        })
        .await
        .expect("close frame missing within timeout")
        .expect("close frame missing after timeout");

        let reason = observed_close;
        assert_eq!(reason.code, CloseCode::Normal);
        assert_eq!(reason.description.as_deref(), Some("heartbeat timeout"));
    }
}
