//! Domain port for route queue enqueue metrics.
//!
//! This port keeps queue observability behind the domain boundary. Queue
//! adapters can record enqueue outcomes without depending on Prometheus or any
//! other metrics backend, while tests can inject a no-op implementation.

use std::time::Duration;

/// Bounded route queue enqueue outcomes used as metrics labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteQueueOutcome {
    /// The job payload was accepted by the queue provider.
    Success,
    /// The queue provider failed to accept the job payload.
    Failure,
}

impl RouteQueueOutcome {
    /// Return the stable metrics label for this outcome.
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
        }
    }
}

/// Metrics recorder for route queue enqueue attempts.
pub trait RouteQueueMetrics: Send + Sync {
    /// Record one route queue enqueue attempt with its outcome and latency.
    fn observe_enqueue(&self, outcome: RouteQueueOutcome, latency: Duration);
}

/// No-op route queue metrics recorder for tests and disabled metrics builds.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoOpRouteQueueMetrics;

impl RouteQueueMetrics for NoOpRouteQueueMetrics {
    fn observe_enqueue(&self, _outcome: RouteQueueOutcome, _latency: Duration) {}
}
