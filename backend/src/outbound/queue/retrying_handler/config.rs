//! Queue-specific policy and dependencies for the retrying Apalis service.

use std::sync::Arc;

use crate::domain::jobs::retry::JobRetryPolicy;
use crate::domain::ports::RouteQueueMetrics;
use crate::outbound::queue::postgres_retry_scheduler::PostgresRetryScheduler;

/// Queue-specific dependencies and policy for a [`super::RetryingJobService`].
pub struct RetryingJobServiceConfig {
    pub(super) queue_name: String,
    pub(super) policy: JobRetryPolicy,
    pub(super) scheduler: PostgresRetryScheduler,
    pub(super) metrics: Arc<dyn RouteQueueMetrics>,
}

impl RetryingJobServiceConfig {
    /// Create the configuration shared by one queue's Apalis service.
    ///
    /// # Examples
    ///
    /// ```text
    /// let config = RetryingJobServiceConfig::new(
    ///     queue_name,
    ///     queue_policy,
    ///     PostgresRetryScheduler::new(pool.clone()),
    ///     metrics,
    /// );
    /// ```
    pub fn new(
        queue_name: String,
        policy: JobRetryPolicy,
        scheduler: PostgresRetryScheduler,
        metrics: Arc<dyn RouteQueueMetrics>,
    ) -> Self {
        Self {
            queue_name,
            policy,
            scheduler,
            metrics,
        }
    }
}
