//! Per-connection WebSocket handler.
//!
//! Keeps WebSocket framing and heartbeats at the edge while deferring
//! application behaviour to the injected domain port (`UserOnboarding`). The public
//! WebSocket contract pings every 5s and considers a connection idle after
//! 10s without client traffic. Tests shorten these intervals to speed up
//! feedback; adjust the constants below if SLAs change so clients and
//! intermediaries stay aligned.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::domain::ports::UserOnboarding;
use crate::domain::{TraceId, UserEvent};
use crate::inbound::ws::messages::{
    DisplayNameRequest, InvalidDisplayNameResponse, UserCreatedResponse,
};
use actix_ws::{CloseCode, CloseReason, Closed, Message, MessageStream, Session};
use tokio::time;
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

pub(super) async fn handle_ws_session(
    onboarding: Arc<dyn UserOnboarding>,
    session: Session,
    stream: MessageStream,
) {
    WsSession::new(onboarding).run(session, stream).await;
}

enum SessionAction {
    Continue,
    Close(Option<CloseReason>),
    Stop,
}

struct WsSession {
    onboarding: Arc<dyn UserOnboarding>,
}

impl WsSession {
    fn new(onboarding: Arc<dyn UserOnboarding>) -> Self {
        Self { onboarding }
    }

    async fn run(&self, mut session: Session, mut stream: MessageStream) {
        let mut last_heartbeat = Instant::now();
        let mut heartbeat = time::interval(HEARTBEAT_INTERVAL);

        loop {
            tokio::select! {
                _ = heartbeat.tick() => {
                    if Instant::now().duration_since(last_heartbeat) > CLIENT_TIMEOUT {
                        warn!("WebSocket heartbeat timeout; closing connection");
                        self.close_with_reason(session, CloseCode::Normal, "heartbeat timeout").await;
                        return;
                    }

                    if let Err(error) = session.ping(b"").await {
                        warn!(error = %error, "Failed to send WebSocket ping");
                        return;
                    }
                }
                message = stream.recv() => {
                    let Some(message) = message else {
                        return;
                    };

                    match message {
                        Ok(message) => {
                            match self.handle_message(&mut session, &mut last_heartbeat, message).await {
                                SessionAction::Continue => {}
                                SessionAction::Stop => return,
                                SessionAction::Close(reason) => {
                                    self.close_session(session, reason).await;
                                    return;
                                }
                            }
                        }
                        Err(error) => {
                            warn!(error = %error, "WebSocket protocol error");
                            self.close_with_reason(session, CloseCode::Protocol, "protocol error").await;
                            return;
                        }
                    }
                }
            }
        }
    }

    async fn handle_message(
        &self,
        session: &mut Session,
        last_heartbeat: &mut Instant,
        message: Message,
    ) -> SessionAction {
        match message {
            Message::Ping(payload) => {
                *last_heartbeat = Instant::now();
                if let Err(error) = session.pong(&payload).await {
                    warn!(error = %error, "Failed to pong WebSocket client");
                    return SessionAction::Stop;
                }
            }
            Message::Text(text) => {
                *last_heartbeat = Instant::now();
                return self.handle_text_message(session, text.as_ref()).await;
            }
            Message::Pong(_) | Message::Binary(_) | Message::Continuation(_) | Message::Nop => {
                *last_heartbeat = Instant::now();
            }
            Message::Close(reason) => return SessionAction::Close(reason),
        }

        SessionAction::Continue
    }

    async fn handle_text_message(&self, session: &mut Session, text: &str) -> SessionAction {
        let request = match serde_json::from_str::<DisplayNameRequest>(text) {
            Ok(request) => request,
            Err(error) => {
                warn!(error = %error, "Rejected malformed WebSocket payload");
                return SessionAction::Close(Some(CloseReason {
                    code: CloseCode::Policy,
                    description: Some("invalid payload".to_owned()),
                }));
            }
        };

        let event = self.handle_display_name_request(request);
        if let Err(error) = self.handle_user_event(session, event).await {
            warn!(error = %error, "WebSocket session closed while sending message");
            return SessionAction::Stop;
        }

        SessionAction::Continue
    }

    fn handle_display_name_request(&self, request: DisplayNameRequest) -> UserEvent {
        let trace_id = TraceId::from_uuid(request.trace_id);
        // `register` must remain CPU-bound; any I/O work should be offloaded to other tasks.
        self.onboarding.register(trace_id, request.display_name)
    }

    async fn handle_user_event(
        &self,
        session: &mut Session,
        event: UserEvent,
    ) -> Result<(), Closed> {
        match event {
            UserEvent::UserCreated(event) => {
                let response: UserCreatedResponse = event.into();
                self.send_json(session, &response).await
            }
            UserEvent::DisplayNameRejected(event) => {
                let response: InvalidDisplayNameResponse = event.into();
                self.send_json(session, &response).await
            }
        }
    }

    async fn send_json<T: serde::Serialize>(
        &self,
        session: &mut Session,
        payload: &T,
    ) -> Result<(), Closed> {
        match serde_json::to_string(payload) {
            Ok(body) => session.text(body).await,
            Err(error) => {
                // In debug builds fail fast so schema drift is fixed; in release we log and keep the connection alive.
                if cfg!(debug_assertions) {
                    panic!("domain events must serialize: {error}");
                } else {
                    warn!(error = %error, "Failed to serialize WebSocket payload");
                }
                Ok(())
            }
        }
    }

    async fn close_with_reason(
        &self,
        session: Session,
        code: CloseCode,
        description: &'static str,
    ) {
        let reason = CloseReason {
            code,
            description: Some(description.to_owned()),
        };
        self.close_session(session, Some(reason)).await;
    }

    async fn close_session(&self, session: Session, reason: Option<CloseReason>) {
        if let Err(error) = session.close(reason).await {
            warn!(error = %error, "Failed to close WebSocket session");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::UserOnboardingService;
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
