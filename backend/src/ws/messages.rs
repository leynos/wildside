//! WebSocket message types for user events.
use actix::Message;
use serde::Serialize;
use uuid::Uuid;

/// Event emitted when a new user is created.
#[derive(Debug, Serialize, Message)]
#[rtype(result = "()")]
pub struct UserCreated {
    #[serde(rename = "trace_id")]
    pub trace_id: String,
    pub id: String,
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
        let event = UserCreated::new("123", "Alice");
        let value = serde_json::to_value(&event).unwrap();
        assert!(value.get("trace_id").is_some());
        assert_eq!(value.get("id").and_then(Value::as_str), Some("123"));
        assert_eq!(
            value.get("display_name").and_then(Value::as_str),
            Some("Alice")
        );
        insta::assert_json_snapshot!(value, {
            ".trace_id" => "[trace_id]"
        });
    }
}
