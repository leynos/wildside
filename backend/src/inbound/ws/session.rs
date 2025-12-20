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

enum SessionError {
    ClientClosed(Option<CloseReason>),
    StreamClosed,
    HeartbeatTimeout,
    Protocol(ProtocolError),
    InvalidPayload,
    Network(Closed),
}

enum CloseAction {
    None,
    Close(Option<CloseReason>),
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
            let result = tokio::select! {
                _ = heartbeat.tick() => {
                    self.handle_heartbeat_tick(&mut session, &last_heartbeat).await
                }
                message = stream.recv() => {
                    self.handle_stream_message(&mut session, &mut last_heartbeat, message)
                        .await
                }
            };

            if let Err(error) = result {
                self.log_shutdown_reason(&error);
                let close_action = self.close_action_for(&error);
                self.close_session_if_needed(session, close_action).await;
                return;
            }
        }
    }

    async fn handle_heartbeat_tick(
        &self,
        session: &mut Session,
        last_heartbeat: &Instant,
    ) -> Result<(), SessionError> {
        if Instant::now().duration_since(*last_heartbeat) > CLIENT_TIMEOUT {
            return Err(SessionError::HeartbeatTimeout);
        }

        session.ping(b"").await.map_err(SessionError::Network)
    }

    async fn handle_stream_message(
        &self,
        session: &mut Session,
        last_heartbeat: &mut Instant,
        message: Option<Result<Message, ProtocolError>>,
    ) -> Result<(), SessionError> {
        let Some(message) = message else {
            return Err(SessionError::StreamClosed);
        };

        match message {
            Ok(message) => self.handle_message(session, last_heartbeat, message).await,
            Err(error) => Err(SessionError::Protocol(error)),
        }
    }

    async fn handle_message(
        &self,
        session: &mut Session,
        last_heartbeat: &mut Instant,
        message: Message,
    ) -> Result<(), SessionError> {
        match message {
            Message::Ping(payload) => {
                *last_heartbeat = Instant::now();
                session
                    .pong(&payload)
                    .await
                    .map_err(SessionError::Network)?;
                Ok(())
            }
            Message::Text(text) => {
                *last_heartbeat = Instant::now();
                self.handle_text_message(session, text.as_ref()).await
            }
            Message::Pong(_) | Message::Binary(_) | Message::Continuation(_) | Message::Nop => {
                *last_heartbeat = Instant::now();
                Ok(())
            }
            Message::Close(reason) => Err(SessionError::ClientClosed(reason)),
        }
    }

    async fn handle_text_message(
        &self,
        session: &mut Session,
        text: &str,
    ) -> Result<(), SessionError> {
        let request = match serde_json::from_str::<DisplayNameRequest>(text) {
            Ok(request) => request,
            Err(error) => {
                warn!(error = %error, "Rejected malformed WebSocket payload");
                return Err(SessionError::InvalidPayload);
            }
        };

        let event = self.handle_display_name_request(request);
        self.handle_user_event(session, event)
            .await
            .map_err(SessionError::Network)
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

    fn log_shutdown_reason(&self, error: &SessionError) {
        match error {
            SessionError::HeartbeatTimeout => {
                warn!("WebSocket heartbeat timeout; closing connection");
            }
            SessionError::Protocol(error) => {
                warn!(error = %error, "WebSocket protocol error");
            }
            SessionError::Network(error) => {
                warn!(error = %error, "WebSocket send failed; closing connection");
            }
            SessionError::InvalidPayload
            | SessionError::ClientClosed(_)
            | SessionError::StreamClosed => {}
        }
    }

    fn close_action_for(&self, error: &SessionError) -> CloseAction {
        match error {
            SessionError::HeartbeatTimeout => CloseAction::Close(Some(CloseReason {
                code: CloseCode::Normal,
                description: Some("heartbeat timeout".to_owned()),
            })),
            SessionError::Protocol(_) => CloseAction::Close(Some(CloseReason {
                code: CloseCode::Protocol,
                description: Some("protocol error".to_owned()),
            })),
            SessionError::InvalidPayload => CloseAction::Close(Some(CloseReason {
                code: CloseCode::Policy,
                description: Some("invalid payload".to_owned()),
            })),
            SessionError::ClientClosed(reason) => CloseAction::Close(reason.clone()),
            SessionError::StreamClosed | SessionError::Network(_) => CloseAction::None,
        }
    }

    async fn close_session_if_needed(&self, session: Session, close_action: CloseAction) {
        if let CloseAction::Close(reason) = close_action {
            if let Err(error) = session.close(reason).await {
                warn!(error = %error, "Failed to close WebSocket session");
            }
        }
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
