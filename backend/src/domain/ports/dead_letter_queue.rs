//! Domain port for inspecting and managing jobs after a terminal outcome.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::define_port_error;

define_port_error! {
    /// Failures returned by dead-letter storage operations.
    pub enum DeadLetterError {
        /// The requested job is absent or no longer satisfies the dead predicate.
        NotDead { message: String } => "dead job is not dead: {message}",
        /// The dead job's queue has no configured retry budget.
        UnknownQueue { message: String } => "dead job queue has no retry budget: {message}",
        /// PostgreSQL could not complete the operation.
        Unavailable { message: String } => "dead-letter storage is unavailable: {message}",
    }
}

/// Opaque identifier for a dead job; its representation belongs to the adapter.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DeadJobId(String);

impl DeadJobId {
    /// Preserve a storage identifier without exposing its format to callers.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Return the identifier in the representation required by the adapter.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Classification of a terminal job outcome.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeadReason {
    /// A retryable failure reached the queue's configured attempt limit.
    Exhausted,
    /// A handler classified the failure as permanent.
    Fatal,
    /// A legacy or malformed row has no recognized terminal reason.
    Unclassified,
}

/// A job parked after a terminal queue outcome.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeadJob {
    pub id: DeadJobId,
    pub job_type: String,
    pub reason: DeadReason,
    pub attempts: u32,
    /// A bounded error string, without the serialized job payload.
    pub last_error: Option<String>,
    /// Number of operator requeues recorded in D15 metadata.
    pub requeue_count: u32,
    pub dead_at: Option<DateTime<Utc>>,
}

/// Read and manage terminal jobs without exposing PostgreSQL to the domain.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait DeadLetterQueue: Send + Sync {
    /// List newest dead jobs first, returning at most `limit` entries.
    async fn list_dead(&self, limit: u32) -> Result<Vec<DeadJob>, DeadLetterError>;

    /// Restore the configured retry budget for a currently dead job.
    async fn requeue(&self, id: &DeadJobId) -> Result<(), DeadLetterError>;

    /// Permanently remove a currently dead job.
    async fn discard(&self, id: &DeadJobId) -> Result<(), DeadLetterError>;
}

#[cfg(test)]
mod tests;
