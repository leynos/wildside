//! WebSocket message types emitted by the backend (e.g., UserCreated).
use actix::Message;
use serde::Serialize;
use uuid::Uuid;

/// Payload emitted when a new user is created.
#[derive(Debug, Serialize, Message)]
#[rtype(result = "()")]
pub struct UserCreated {
    /// Correlation identifier for cross-service tracing.
    pub trace_id: String,
    /// The user's unique identifier.
    pub id: String,
    /// The user's chosen display name.
    pub display_name: String,
}

impl UserCreated {
    /// Construct with a fresh trace identifier.
    pub fn new(id: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            trace_id: Uuid::new_v4().to_string(),
            id: id.into(),
            display_name: display_name.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serde_json::Value;

    #[rstest]
    fn serializes_user_created() {
        let msg = UserCreated::new("123", "Alice");
        let value = serde_json::to_value(&msg).unwrap();
        assert!(value.get("trace_id").is_some());
        assert_eq!(value.get("id").and_then(Value::as_str), Some("123"));
        assert_eq!(
            value.get("display_name").and_then(Value::as_str),
            Some("Alice")
        );
        insta::assert_json_snapshot!(value, { ".trace_id" => "[trace_id]" });
    }
}
