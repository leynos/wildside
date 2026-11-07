//! Apalis service that applies the domain retry policy to one job delivery.

use std::future::Future;
use std::marker::PhantomData;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::task::{Context, Poll};

use apalis_core::{error::BoxDynError, layers::Service};
use apalis_postgres::PgTask;
use futures_util::future::BoxFuture;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tracing::Instrument;

mod config;
mod decision;
mod failure;

pub use config::{JitterSource, RetryingJobServiceConfig, ThreadRngJitter};
use failure::{RetryFailureContext, current_attempt, handle_failure, running_worker_id};

use crate::domain::jobs::retry::{JobErrorKind, JobFailure, JobRetryPolicy};
use crate::domain::ports::RouteQueueMetrics;
use crate::outbound::queue::RetryPersistence;
use crate::outbound::queue::decode_job;

/// A queue-specific Apalis service that schedules retries in PostgreSQL.
///
/// `J` is the queue's concrete `v1` job envelope. Decoding into it at this
/// boundary ensures the handler only ever sees a payload that validated against
/// the job type's required fields, not merely one with a supported version tag.
pub struct RetryingJobService<J, F> {
    queue_name: String,
    policy: JobRetryPolicy,
    scheduler: Arc<dyn RetryPersistence>,
    metrics: Arc<dyn RouteQueueMetrics>,
    jitter: Arc<dyn JitterSource>,
    handler: F,
    job: PhantomData<fn() -> J>,
}

impl<J, F> RetryingJobService<J, F> {
    /// Build the production wrapper for one queue and its retry policy.
    ///
    /// The handler receives the decoded v1 job envelope and current Apalis
    /// attempt. The wrapper records retry scheduling before returning a
    /// bounded error for the PostgreSQL acknowledgement.
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
    /// let service = RetryingJobService::<GenerateRouteJob, _>::new(config, handle_job);
    /// let worker = WorkerBuilder::new(worker_name)
    ///     .backend(postgres_storage)
    ///     .build(service);
    /// ```
    pub fn new(config: RetryingJobServiceConfig, handler: F) -> Self {
        Self {
            queue_name: config.queue_name,
            policy: config.policy,
            scheduler: config.scheduler,
            metrics: config.metrics,
            jitter: config.jitter,
            handler,
            job: PhantomData,
        }
    }
}

impl<J, F, Fut> Service<PgTask<Value>> for RetryingJobService<J, F>
where
    J: DeserializeOwned + Send + 'static,
    F: Fn(J, NonZeroU32) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Result<(), JobFailure>> + Send + 'static,
{
    type Response = ();
    type Error = BoxDynError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    /// Decode one delivery, invoke the handler, and persist its failure.
    ///
    /// The span records the identity facts only after decoding the task, so a
    /// delivery that never reaches the handler still carries its ids.
    #[tracing::instrument(
        skip_all,
        fields(
            operation = "call",
            queue = self.queue_name,
            task_id = tracing::field::Empty,
            worker_id = tracing::field::Empty,
            attempt = tracing::field::Empty,
        )
    )]
    fn call(&mut self, task: PgTask<Value>) -> Self::Future {
        let policy = self.policy.clone();
        let scheduler = self.scheduler.clone();
        let metrics = std::sync::Arc::clone(&self.metrics);
        let jitter = std::sync::Arc::clone(&self.jitter);
        let queue_name = self.queue_name.clone();
        let handler = self.handler.clone();
        let span = tracing::Span::current();

        let future = async move {
            let attempt = current_attempt(task.parts.attempt.current());
            let context_max_attempts =
                u32::try_from(task.parts.ctx.max_attempts()).unwrap_or_default();
            let worker_id = running_worker_id(&task);
            let task_id = task.parts.task_id.as_ref().map(ToString::to_string);
            tracing::Span::current().record("attempt", tracing::field::display(attempt));
            tracing::Span::current().record("task_id", tracing::field::debug(task_id.as_deref()));
            tracing::Span::current()
                .record("worker_id", tracing::field::debug(worker_id.as_deref()));
            let failure_context = RetryFailureContext {
                attempt,
                context_max_attempts,
                policy: &policy,
                scheduler: scheduler.as_ref(),
                metrics: metrics.as_ref(),
                jitter: jitter.as_ref(),
                queue_name: &queue_name,
                task_id: task_id.as_deref(),
                worker_id: worker_id.as_deref(),
            };

            let payload = match decode_job::<J>(&task.args) {
                Ok(payload) => payload,
                Err(error) => {
                    let failure = JobFailure::new(JobErrorKind::Fatal, error.to_string());
                    return handle_failure(failure, &failure_context).await;
                }
            };

            match handler(payload, attempt).await {
                Ok(()) => Ok(()),
                Err(failure) => handle_failure(failure, &failure_context).await,
            }
        };

        Box::pin(future.instrument(span))
    }
}

#[cfg(test)]
mod tests;
