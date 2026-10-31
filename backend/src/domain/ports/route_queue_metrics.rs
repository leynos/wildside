//! Domain port for route queue enqueue, retry, and dead-letter metrics.
//!
//! This port keeps queue observability behind the domain boundary. Queue
//! adapters can record bounded queue outcomes without depending on Prometheus
//! or any other metrics backend, while tests can inject a no-op implementation.

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

/// Bounded outcomes for durable retry scheduling metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteRetryOutcome {
    /// The task's persisted `run_at` was updated successfully.
    Scheduled,
    /// The guarded scheduling update failed or matched no running task.
    ScheduleFailure,
}

impl RouteRetryOutcome {
    /// Return the stable metrics label for this outcome.
    ///
    /// # Examples
    ///
    /// ```
    /// use backend::domain::ports::RouteRetryOutcome;
    ///
    /// assert_eq!(RouteRetryOutcome::Scheduled.as_label(), "scheduled");
    /// ```
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Scheduled => "scheduled",
            Self::ScheduleFailure => "schedule_failure",
        }
    }
}

/// Bounded reasons for durable dead-letter metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteDeadLetterOutcome {
    /// A retryable failure reached the configured attempt limit.
    Exhausted,
    /// A handler reported a permanent failure.
    Fatal,
}

impl RouteDeadLetterOutcome {
    /// Return the stable metrics label for this outcome.
    ///
    /// # Examples
    ///
    /// ```
    /// use backend::domain::ports::RouteDeadLetterOutcome;
    ///
    /// assert_eq!(RouteDeadLetterOutcome::Fatal.as_label(), "fatal");
    /// ```
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Exhausted => "exhausted",
            Self::Fatal => "fatal",
        }
    }
}

/// Metrics recorder for route queue enqueue and retry outcomes.
pub trait RouteQueueMetrics: Send + Sync {
    /// Record one route queue enqueue attempt with its outcome and latency.
    fn observe_enqueue(&self, outcome: RouteQueueOutcome, latency: Duration);

    /// Record a durable retry scheduling outcome.
    fn observe_retry(&self, outcome: RouteRetryOutcome);

    /// Record a terminal dead-letter reason.
    fn observe_dead_letter(&self, outcome: RouteDeadLetterOutcome);
}

/// No-op route queue metrics recorder for tests and disabled metrics builds.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoOpRouteQueueMetrics;

impl RouteQueueMetrics for NoOpRouteQueueMetrics {
    fn observe_enqueue(&self, _outcome: RouteQueueOutcome, _latency: Duration) {}

    fn observe_retry(&self, _outcome: RouteRetryOutcome) {}

    fn observe_dead_letter(&self, _outcome: RouteDeadLetterOutcome) {}
}
