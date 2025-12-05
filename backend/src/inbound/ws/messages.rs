//! Wire-level message definitions for the WebSocket adapter.
//!
//! Domain events are transformed into these payloads before being serialized
//! to JSON and sent to connected clients.

use crate::domain::{DisplayNameRejectedEvent, DisplayNameSubmission, UserCreatedEvent};
use crate::middleware::trace::TraceId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Inbound request payload provided by the client.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayNameRequest {
    /// Client-provided correlation identifier.
    pub trace_id: Uuid,
    /// Desired display name.
    #[serde(alias = "display_name")]
    pub display_name: String,
}

impl From<DisplayNameRequest> for DisplayNameSubmission {
    fn from(value: DisplayNameRequest) -> Self {
        Self::new(TraceId::from_uuid(value.trace_id), value.display_name)
    }
}

/// Generic envelope attaching a correlation identifier to an outbound payload.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope<T> {
    trace_id: Uuid,
    #[serde(flatten)]
    payload: T,
}

impl<T> Envelope<T> {
    /// Construct with the provided trace identifier.
    pub fn with_trace_id(trace_id: TraceId, payload: T) -> Self {
        Self {
            trace_id: *trace_id.as_uuid(),
            payload,
        }
    }
}

/// Outbound payload emitted when a user is created.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserCreatedPayload {
    /// Unique user identifier.
    pub id: String,
    /// User's chosen display name.
    pub display_name: String,
}

impl From<UserCreatedEvent> for Envelope<UserCreatedPayload> {
    fn from(value: UserCreatedEvent) -> Self {
        let payload = UserCreatedPayload {
            id: value.user.id().to_string(),
            display_name: value.user.display_name().as_ref().to_owned(),
        };
        Envelope::with_trace_id(value.trace_id, payload)
    }
}

/// Structured details for invalid display name responses.
#[derive(Debug, Serialize)]
pub struct InvalidDisplayNameDetails {
    field: &'static str,
    value: String,
    message: String,
    code: String,
}

/// Outbound payload describing display name validation failures.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvalidDisplayNamePayload {
    code: String,
    error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<InvalidDisplayNameDetails>,
}

impl From<DisplayNameRejectedEvent> for Envelope<InvalidDisplayNamePayload> {
    fn from(value: DisplayNameRejectedEvent) -> Self {
        let details = InvalidDisplayNameDetails {
            field: value.field(),
            value: value.attempted_name.clone(),
            message: value.message.clone(),
            code: value.reason.code().to_owned(),
        };
        let payload = InvalidDisplayNamePayload {
            code: value.reason.code().to_owned(),
            error: value.message,
            details: Some(details),
        };
        Envelope::with_trace_id(value.trace_id, payload)
    }
}

/// Actix message wrapper carrying domain events for WebSocket sessions.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DisplayName, User, UserId};
    use insta::assert_json_snapshot;
    use rstest::rstest;

    #[rstest]
    fn serialises_user_created_event() {
        let user = User::new(
            UserId::new("3fa85f64-5717-4562-b3fc-2c963f66afa6").expect("valid UUID"),
            DisplayName::new("Alice").expect("valid display name"),
        );
        let event = UserCreatedEvent {
            trace_id: TraceId::from_uuid(Uuid::nil()),
            user,
        };
        let envelope: Envelope<UserCreatedPayload> = event.into();
        assert_json_snapshot!(envelope);
    }

    #[rstest]
    fn serialises_invalid_display_name_event() {
        let reason = crate::domain::DisplayNameRejectionReason::InvalidCharacters;
        let event = DisplayNameRejectedEvent {
            trace_id: TraceId::from_uuid(Uuid::nil()),
            attempted_name: "bad$char".into(),
            reason,
            message: reason.message(),
        };
        let envelope: Envelope<InvalidDisplayNamePayload> = event.into();
        assert_json_snapshot!(envelope);
    }
}
