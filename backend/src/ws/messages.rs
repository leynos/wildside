use actix::Message;
use serde::Serialize;
use uuid::Uuid;

/// Trait for correlated messages that expose a trace identifier.
pub trait Correlated {
    fn trace_id(&self) -> &str;
}

/// Generic correlated message envelope carrying headers and payload.
#[derive(Debug, Serialize)]
pub struct Envelope<T> {
    /// Correlation identifier propagated to the client.
    #[serde(rename = "trace_id")]
    trace_id: String,
    #[serde(flatten)]
    payload: T,
}

impl<T> Envelope<T> {
    /// Wrap the given payload and assign a new trace identifier.
    pub fn new(payload: T) -> Self {
        Self {
            trace_id: Uuid::new_v4().to_string(),
            payload,
        }
    }
}

impl<T> Correlated for Envelope<T> {
    fn trace_id(&self) -> &str {
        &self.trace_id
    }
}

impl<T> Message for Envelope<T>
where
    T: Serialize + Send + 'static,
{
    type Result = ();
}

/// Payload emitted when a new user is created.
#[derive(Debug, Serialize)]
pub struct UserCreated {
    pub id: String,
    pub display_name: String,
}

/// Alias for a correlated UserCreated event.
pub type UserCreatedMessage = Envelope<UserCreated>;
