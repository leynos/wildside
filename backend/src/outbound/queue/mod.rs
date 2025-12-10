//! Placeholder for future Apalis job queue adapter.
//!
//! This module provides a stub implementation of the `RouteQueue` port that
//! discards all enqueued jobs. It serves as a structural placeholder until
//! the Apalis-backed implementation is completed.
//!
//! # Future Implementation
//!
//! The full Apalis implementation will:
//! - Use `apalis` with PostgreSQL or Redis as the message broker
//! - Define job structs (`GenerateRouteJob`, `EnrichmentJob`)
//! - Implement retry policies with exponential backoff
//! - Support dead-letter handling for failed jobs
//! - Propagate trace IDs through job metadata for observability
//!
//! # Roadmap
//!
//! See `docs/backend-roadmap.md` for the Apalis queue implementation tasks.

use async_trait::async_trait;

use crate::domain::ports::{JobDispatchError, RouteQueue};

/// Stub queue implementation that discards all jobs.
///
/// This placeholder implements the `RouteQueue` port with no-op behaviour,
/// allowing the application to compile and run without a job queue backend.
/// All `enqueue` operations succeed but the job is not persisted or processed.
#[derive(Debug, Clone, Default)]
pub struct StubRouteQueue;

impl StubRouteQueue {
    /// Create a new stub queue instance.
    pub fn new() -> Self {
        Self
    }
}

/// Placeholder plan type for the stub implementation.
///
/// The concrete implementation will use the domain's actual `RoutePlan` type
/// once that type is defined. This marker satisfies the trait's associated
/// type requirement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StubPlan;

#[async_trait]
impl RouteQueue for StubRouteQueue {
    type Plan = StubPlan;

    async fn enqueue(&self, _plan: &Self::Plan) -> Result<(), JobDispatchError> {
        // Stub discards jobs; real implementation will submit to Apalis.
        // Log a warning so developers notice if this stub is used unintentionally.
        tracing::warn!("StubRouteQueue: job discarded (queue adapter not implemented)");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[tokio::test]
    async fn stub_queue_enqueue_succeeds() {
        let queue = StubRouteQueue::new();
        let plan = StubPlan;

        let result = queue.enqueue(&plan).await;
        assert!(result.is_ok(), "stub queue enqueue should succeed");
    }
}
