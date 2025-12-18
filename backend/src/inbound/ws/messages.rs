//! Wire-level message definitions for the WebSocket adapter.
//!
//! Domain events are transformed into these payloads before being serialized
//! to JSON and sent to connected clients.

use crate::domain::{DisplayNameRejectedEvent, UserCreatedEvent};
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

/// Outbound payload emitted when a user is created.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserCreatedResponse {
    pub trace_id: Uuid,
    /// Unique user identifier.
    pub id: String,
    /// User's chosen display name.
    pub display_name: String,
}

impl From<UserCreatedEvent> for UserCreatedResponse {
    fn from(value: UserCreatedEvent) -> Self {
        Self {
            trace_id: *value.trace_id.as_uuid(),
            id: value.user.id().to_string(),
            display_name: value.user.display_name().as_ref().to_owned(),
        }
    }
}

/// Structured details for invalid display name responses.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvalidDisplayNameDetails {
    field: &'static str,
    value: String,
    message: String,
    code: String,
}

impl From<&DisplayNameRejectedEvent> for InvalidDisplayNameDetails {
    fn from(value: &DisplayNameRejectedEvent) -> Self {
        let (code, message, field) = value.error.display_name_error_meta().unwrap_or((
            "invalid_display_name",
            value.error.to_string(),
            value.field(),
        ));

        Self {
            field,
            value: value.attempted_name.clone(),
            message,
            code: code.to_owned(),
        }
    }
}

/// Outbound payload describing display name validation failures.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvalidDisplayNameResponse {
    pub trace_id: Uuid,
    pub code: String,
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<InvalidDisplayNameDetails>,
}

impl From<DisplayNameRejectedEvent> for InvalidDisplayNameResponse {
    fn from(value: DisplayNameRejectedEvent) -> Self {
        let (code, message, _field) = value.error.display_name_error_meta().unwrap_or((
            "invalid_display_name",
            value.error.to_string(),
            value.field(),
        ));

        let details = value
            .error
            .display_name_error_meta()
            .map(|_| InvalidDisplayNameDetails::from(&value));

        Self {
            trace_id: *value.trace_id.as_uuid(),
            code: code.to_owned(),
            error: message,
            details,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::TraceId;
    use crate::domain::{DisplayName, User, UserId, UserValidationError};
    use insta::assert_json_snapshot;
    use rstest::rstest;

    #[rstest]
    fn serialises_user_created_event() {
        let user = User::new(
            UserId::new("3fa85f64-5717-4562-b3fc-2c963f66afa6")
                .expect("static test UUID must be valid"),
            DisplayName::new("Alice").expect("static test display name must be valid"),
        );
        let event = UserCreatedEvent {
            trace_id: TraceId::from_uuid(Uuid::nil()),
            user,
        };
        let response: UserCreatedResponse = event.into();
        assert_json_snapshot!(response);
    }

    #[rstest]
    fn serialises_invalid_display_name_event() {
        let event = DisplayNameRejectedEvent {
            trace_id: TraceId::from_uuid(Uuid::nil()),
            attempted_name: "bad$char".into(),
            error: UserValidationError::DisplayNameInvalidCharacters,
        };
        let response: InvalidDisplayNameResponse = event.into();
        assert_json_snapshot!(response);
    }
}
