//! Queue-specific policy and dependencies for the retrying Apalis service.

use std::sync::Arc;

use rand::Rng;

use crate::domain::jobs::retry::{JitterSample, JobRetryPolicy};
use crate::domain::ports::RouteQueueMetrics;
use crate::outbound::queue::RetryPersistence;

/// Source of the jitter sample applied to one retry delay.
///
/// The wrapper asks for exactly one sample per failed delivery. Production
/// uses [`ThreadRngJitter`]; tests supply a fixed sample so the scheduled delay
/// is deterministic without depending on a zero jitter fraction.
pub trait JitterSource: Send + Sync {
    /// Return a sample in the inclusive range `0.0..=1.0`.
    fn sample(&self) -> JitterSample;
}

/// Production [`JitterSource`] backed by the thread-local random generator.
#[derive(Debug, Default, Clone, Copy)]
pub struct ThreadRngJitter;

impl JitterSource for ThreadRngJitter {
    fn sample(&self) -> JitterSample {
        let sample = rand::thread_rng().gen_range(0.0..=1.0);
        JitterSample::new(sample).unwrap_or_else(|_| JitterSample::midpoint())
    }
}

/// Queue-specific dependencies and policy for a [`super::RetryingJobService`].
pub struct RetryingJobServiceConfig {
    pub(super) queue_name: String,
    pub(super) policy: JobRetryPolicy,
    pub(super) scheduler: Arc<dyn RetryPersistence>,
    pub(super) metrics: Arc<dyn RouteQueueMetrics>,
    pub(super) jitter: Arc<dyn JitterSource>,
}

impl RetryingJobServiceConfig {
    /// Create the configuration shared by one queue's Apalis service.
    ///
    /// The jitter source defaults to `ThreadRngJitter`. Use
    /// [`Self::with_jitter`] to supply a deterministic source in tests.
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
            jitter: Arc::new(ThreadRngJitter),
        }
    }

    /// Replace the jitter source.
    ///
    /// # Examples
    ///
    /// ```text
    /// let config = RetryingJobServiceConfig::new(queue, policy, scheduler, metrics)
    ///     .with_jitter(Arc::new(FixedJitter(JitterSample::midpoint())));
    /// ```
    #[must_use]
    pub fn with_jitter(mut self, jitter: Arc<dyn JitterSource>) -> Self {
        self.jitter = jitter;
        self
    }
}
