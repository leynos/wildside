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
use actix_ws::{CloseCode, CloseReason, Closed, Message, MessageStream, ProtocolError, Session};
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
                    if self
                        .handle_heartbeat_tick(&mut session, &last_heartbeat)
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
                message = stream.recv() => {
                    if self
                        .handle_stream_message(&mut session, &mut last_heartbeat, message)
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
            }
        }
    }

    async fn handle_heartbeat_tick(
        &self,
        session: &mut Session,
        last_heartbeat: &Instant,
    ) -> Result<(), ()> {
        if Instant::now().duration_since(*last_heartbeat) > CLIENT_TIMEOUT {
            warn!("WebSocket heartbeat timeout; closing connection");
            self.close_with_reason(session, CloseCode::Normal, "heartbeat timeout")
                .await;
            return Err(());
        }

        if let Err(error) = session.ping(b"").await {
            warn!(error = %error, "Failed to send WebSocket ping");
            return Err(());
        }

        Ok(())
    }

    async fn handle_stream_message(
        &self,
        session: &mut Session,
        last_heartbeat: &mut Instant,
        message: Option<Result<Message, ProtocolError>>,
    ) -> Result<(), ()> {
        let Some(message) = message else {
            return Err(());
        };

        match message {
            Ok(message) => match self.handle_message(session, last_heartbeat, message).await {
                SessionAction::Continue => Ok(()),
                SessionAction::Stop => Err(()),
                SessionAction::Close(reason) => {
                    self.close_session(session, reason).await;
                    Err(())
                }
            },
            Err(error) => {
                warn!(error = %error, "WebSocket protocol error");
                self.close_with_reason(session, CloseCode::Protocol, "protocol error")
                    .await;
                Err(())
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
        session: &mut Session,
        code: CloseCode,
        description: &'static str,
    ) {
        let reason = CloseReason {
            code,
            description: Some(description.to_owned()),
        };
        self.close_session(session, Some(reason)).await;
    }

    async fn close_session(&self, session: &mut Session, reason: Option<CloseReason>) {
        let session = session.clone();
        if let Err(error) = session.close(reason).await {
            warn!(error = %error, "Failed to close WebSocket session");
        }
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
