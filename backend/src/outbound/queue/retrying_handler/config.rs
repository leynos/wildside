//! Queue-specific policy and dependencies for the retrying Apalis service.

use std::sync::Arc;

use crate::domain::jobs::retry::JobRetryPolicy;
use crate::domain::ports::RouteQueueMetrics;
use crate::outbound::queue::RetryPersistence;

/// Queue-specific dependencies and policy for a [`super::RetryingJobService`].
pub struct RetryingJobServiceConfig {
    pub(super) queue_name: String,
    pub(super) policy: JobRetryPolicy,
    pub(super) scheduler: Arc<dyn RetryPersistence>,
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
    ///     Arc::new(PostgresRetryScheduler::new(pool.clone())),
    ///     metrics,
    /// );
    /// ```
    pub fn new(
        queue_name: String,
        policy: JobRetryPolicy,
        scheduler: Arc<dyn RetryPersistence>,
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
