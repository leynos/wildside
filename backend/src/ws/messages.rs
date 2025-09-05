use actix::Message;
use serde::Serialize;
use uuid::Uuid;

/// Payload emitted when a new user is created.
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
