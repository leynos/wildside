//! Domain port describing queue dispatch semantics for route jobs.
use async_trait::async_trait;

use super::define_port_error;

define_port_error! {
    /// Errors surfaced by the queue/dispatcher adapter.
    pub enum JobDispatchError {
        /// Queue infrastructure is unavailable.
        Unavailable { message: String } => "route queue is unavailable: {message}",
        /// The job could not be acknowledged or persisted.
        Rejected { message: String } => "route job was rejected: {message}",
    }
}

#[async_trait]
pub trait RouteQueue: Send + Sync {
    /// Domain-specific plan representation shared across adapters.
    type Plan: Send + Sync;

    /// Enqueue a plan for downstream processing.
    async fn enqueue(&self, plan: &Self::Plan) -> Result<(), JobDispatchError>;
}
