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

use std::marker::PhantomData;
use std::sync::Once;

use async_trait::async_trait;

use crate::domain::ports::{JobDispatchError, RouteQueue};

/// Guard to ensure the stub warning is only logged once per process.
static STUB_WARNING_LOGGED: Once = Once::new();

/// Stub queue implementation that discards all jobs.
///
/// This placeholder implements the `RouteQueue` port with no-op behaviour,
/// allowing the application to compile and run without a job queue backend.
/// All `enqueue` operations succeed but the job is not persisted or processed.
///
/// The generic parameter `P` allows this stub to be used with any plan type,
/// enabling transparent substitution when the real Apalis adapter is introduced.
///
/// A warning is logged on first use to alert developers that jobs are being
/// discarded. The warning is gated by a `Once` guard to avoid log flooding.
#[derive(Debug)]
pub struct StubRouteQueue<P> {
    _marker: PhantomData<P>,
}

impl<P> Clone for StubRouteQueue<P> {
    fn clone(&self) -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<P> Default for StubRouteQueue<P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P> StubRouteQueue<P> {
    /// Create a new stub queue instance.
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<P: Send + Sync> RouteQueue for StubRouteQueue<P> {
    type Plan = P;

    async fn enqueue(&self, _plan: &Self::Plan) -> Result<(), JobDispatchError> {
        // Stub discards jobs; real implementation will submit to Apalis.
        // Log a warning once so developers notice if this stub is used unintentionally.
        STUB_WARNING_LOGGED.call_once(|| {
            tracing::warn!("StubRouteQueue: job discarded (queue adapter not implemented)");
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    /// Test plan type for unit tests.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestPlan;

    #[rstest]
    #[tokio::test]
    async fn stub_queue_enqueue_succeeds() {
        let queue: StubRouteQueue<TestPlan> = StubRouteQueue::new();
        let plan = TestPlan;

        let result = queue.enqueue(&plan).await;
        assert!(result.is_ok(), "stub queue enqueue should succeed");
    }
}
