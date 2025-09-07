//! WebSocket message types emitted by the backend (e.g., UserCreated).
use actix::Message;
use serde::Serialize;
use uuid::Uuid;

/// Trait for correlated messages that expose a trace identifier.
pub trait Correlated {
    fn trace_id(&self) -> &Uuid;
}

/// Generic envelope that attaches a correlation identifier.
#[derive(Debug, Serialize, Message)]
#[rtype(result = "()")]
pub struct Envelope<T> {
    #[serde(rename = "trace_id")]
    trace_id: Uuid,
    #[serde(flatten)]
    payload: T,
}

impl<T> Envelope<T> {
    /// Construct with a fresh trace identifier.
    pub fn new(payload: T) -> Self {
        Self {
            trace_id: Uuid::new_v4(),
            payload,
        }
    }

    /// Construct with the provided trace identifier.
    pub fn with_trace_id(trace_id: Uuid, payload: T) -> Self {
        Self { trace_id, payload }
    }
}

impl<T> Correlated for Envelope<T> {
    fn trace_id(&self) -> &Uuid {
        &self.trace_id
    }
}

/// Payload emitted when a new user is created.
#[derive(Debug, Serialize, Message)]
#[rtype(result = "()")]
pub struct UserCreated {
    /// The user's unique identifier.
    pub id: String,
    /// The user's chosen display name.
    pub display_name: String,
}

/// Actix message variant carrying `UserCreated`.
pub type UserCreatedMessage = Envelope<UserCreated>;

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serde_json::Value;

    #[rstest]
    fn serialises_user_created() {
        let msg = UserCreated {
            id: "123".into(),
            display_name: "Alice".into(),
        };
        let value = serde_json::to_value(&msg).expect("failed to convert message to JSON value");
        assert_eq!(value.get("id").and_then(Value::as_str), Some("123"));
        assert_eq!(
            value.get("display_name").and_then(Value::as_str),
            Some("Alice")
        );
        insta::assert_json_snapshot!(value);
    }
}
