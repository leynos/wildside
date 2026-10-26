//! Apalis service that applies the domain retry policy to one job delivery.

use std::future::Future;
use std::num::NonZeroU32;
use std::task::{Context, Poll};
use std::time::Duration;

use apalis_core::{error::BoxDynError, layers::Service, worker::context::WorkerContext};
use apalis_postgres::PgTask;
use futures_util::future::BoxFuture;
use rand::Rng;
use serde_json::Value;
use tracing::{error, warn};

mod config;
mod decision;

pub use config::RetryingJobServiceConfig;
use decision::{RetryDecisionContext, RetryWrapperDecision, decide_retry_wrapper_action};

use crate::domain::jobs::retry::{JitterSample, JobErrorKind, JobFailure, JobRetryPolicy};
use crate::domain::ports::{RouteDeadLetterOutcome, RouteQueueMetrics, RouteRetryOutcome};
use crate::outbound::queue::decode_job;
use crate::outbound::queue::postgres_retry_scheduler::{
    PostgresRetryScheduler, TerminalRetryReason, TerminalRetryUpdate,
};

/// A queue-specific Apalis service that schedules retries in PostgreSQL.
pub struct RetryingJobService<F> {
    queue_name: String,
    policy: JobRetryPolicy,
    scheduler: PostgresRetryScheduler,
    metrics: std::sync::Arc<dyn RouteQueueMetrics>,
    handler: F,
}

impl<F> RetryingJobService<F> {
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
    ///     PostgresRetryScheduler::new(pool.clone()),
    ///     metrics,
    /// );
    /// let service = RetryingJobService::new(config, handle_job);
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
            handler,
        }
    }
}

impl<F, Fut> Service<PgTask<Value>> for RetryingJobService<F>
where
    F: Fn(Value, NonZeroU32) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Result<(), JobFailure>> + Send + 'static,
{
    type Response = ();
    type Error = BoxDynError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, task: PgTask<Value>) -> Self::Future {
        let policy = self.policy.clone();
        let scheduler = self.scheduler.clone();
        let metrics = std::sync::Arc::clone(&self.metrics);
        let queue_name = self.queue_name.clone();
        let handler = self.handler.clone();

        Box::pin(async move {
            let attempt = current_attempt(task.parts.attempt.current());
            let context_max_attempts =
                u32::try_from(task.parts.ctx.max_attempts()).unwrap_or_default();
            let worker_id = running_worker_id(&task);
            let task_id = task.parts.task_id.as_ref().map(ToString::to_string);
            let failure_context = RetryFailureContext {
                attempt,
                context_max_attempts,
                policy: &policy,
                scheduler: &scheduler,
                metrics: metrics.as_ref(),
                queue_name: &queue_name,
                task_id: task_id.as_deref(),
                worker_id: worker_id.as_deref(),
            };

            let payload = match decode_job::<Value>(&task.args) {
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
        })
    }
}

struct RetryFailureContext<'a> {
    attempt: NonZeroU32,
    context_max_attempts: u32,
    policy: &'a JobRetryPolicy,
    scheduler: &'a PostgresRetryScheduler,
    metrics: &'a dyn RouteQueueMetrics,
    queue_name: &'a str,
    task_id: Option<&'a str>,
    worker_id: Option<&'a str>,
}

async fn handle_failure(
    failure: JobFailure,
    context: &RetryFailureContext<'_>,
) -> Result<(), BoxDynError> {
    let kind = failure.kind();
    let jitter = random_jitter();
    let decision = decide_retry_wrapper_action(
        context.policy,
        RetryDecisionContext {
            attempt: context.attempt,
            context_max_attempts: context.context_max_attempts,
            kind,
            jitter,
        },
    );
    let error_message = failure.message().to_owned();
    let identity = match (context.task_id, context.worker_id) {
        (Some(task_id), Some(worker_id)) => RetryJobIdentity { task_id, worker_id },
        _ => {
            record_missing_retry_metadata(context, kind, decision);
            return Err(Box::new(RetryAckError(error_message)));
        }
    };

    match decision {
        RetryWrapperDecision::Schedule { delay } => {
            persist_retry_schedule(context, identity, kind, delay).await;
        }
        RetryWrapperDecision::Terminal {
            reason,
            shrink_limit,
        } => {
            persist_terminal_failure(
                context,
                identity,
                kind,
                TerminalRetryPlan {
                    reason,
                    shrink_limit,
                },
            )
            .await;
        }
    }

    Err(Box::new(RetryAckError(error_message)))
}

struct RetryJobIdentity<'a> {
    task_id: &'a str,
    worker_id: &'a str,
}

#[derive(Clone, Copy)]
struct TerminalRetryPlan {
    reason: TerminalRetryReason,
    shrink_limit: bool,
}

fn record_missing_retry_metadata(
    context: &RetryFailureContext<'_>,
    kind: JobErrorKind,
    decision: RetryWrapperDecision,
) {
    if matches!(decision, RetryWrapperDecision::Schedule { .. }) {
        context
            .metrics
            .observe_retry(RouteRetryOutcome::ScheduleFailure);
    }
    error!(
        queue = context.queue_name,
        attempt = context.attempt.get(),
        error_kind = error_kind_label(kind),
        decision = decision_label(decision),
        "retry metadata missing; cannot persist failure outcome"
    );
}

async fn persist_retry_schedule(
    context: &RetryFailureContext<'_>,
    identity: RetryJobIdentity<'_>,
    kind: JobErrorKind,
    delay: Duration,
) {
    match context
        .scheduler
        .schedule_retry(identity.task_id, identity.worker_id, delay)
        .await
    {
        Ok(true) => {
            context.metrics.observe_retry(RouteRetryOutcome::Scheduled);
            warn!(
                queue = context.queue_name,
                task_id = identity.task_id,
                worker_id = identity.worker_id,
                attempt = context.attempt.get(),
                error_kind = error_kind_label(kind),
                decision = "scheduled",
                delay_micros = delay.as_micros(),
                "job failure scheduled for durable retry"
            );
        }
        Ok(false) => {
            context
                .metrics
                .observe_retry(RouteRetryOutcome::ScheduleFailure);
            warn!(
                queue = context.queue_name,
                task_id = identity.task_id,
                worker_id = identity.worker_id,
                attempt = context.attempt.get(),
                error_kind = error_kind_label(kind),
                decision = "schedule_failure",
                "job retry update no longer owns a running row"
            );
        }
        Err(error) => {
            context
                .metrics
                .observe_retry(RouteRetryOutcome::ScheduleFailure);
            warn!(
                queue = context.queue_name,
                task_id = identity.task_id,
                worker_id = identity.worker_id,
                attempt = context.attempt.get(),
                error_kind = error_kind_label(kind),
                decision = "schedule_failure",
                error = %error,
                "job retry scheduling failed"
            );
        }
    }
}

async fn persist_terminal_failure(
    context: &RetryFailureContext<'_>,
    identity: RetryJobIdentity<'_>,
    kind: JobErrorKind,
    plan: TerminalRetryPlan,
) {
    let TerminalRetryPlan {
        reason,
        shrink_limit,
    } = plan;
    let result = context
        .scheduler
        .record_terminal_failure(TerminalRetryUpdate {
            task_id: identity.task_id,
            worker_id: identity.worker_id,
            attempt: context.attempt,
            reason,
            shrink_limit,
        })
        .await;
    match result {
        Ok(true) => {
            context
                .metrics
                .observe_dead_letter(dead_letter_outcome(reason));
            warn!(
                queue = context.queue_name,
                task_id = identity.task_id,
                worker_id = identity.worker_id,
                attempt = context.attempt.get(),
                error_kind = error_kind_label(kind),
                decision = "dead_lettered",
                reason = reason.as_label(),
                shrink_limit,
                "job failure recorded as terminal"
            );
        }
        Ok(false) => warn!(
            queue = context.queue_name,
            task_id = identity.task_id,
            worker_id = identity.worker_id,
            attempt = context.attempt.get(),
            error_kind = error_kind_label(kind),
            decision = "terminal_write_failure",
            reason = reason.as_label(),
            "job terminal update no longer owns a running row"
        ),
        Err(error) => warn!(
            queue = context.queue_name,
            task_id = identity.task_id,
            worker_id = identity.worker_id,
            attempt = context.attempt.get(),
            error_kind = error_kind_label(kind),
            decision = "terminal_write_failure",
            reason = reason.as_label(),
            error = %error,
            "job terminal update failed"
        ),
    }
}

fn dead_letter_outcome(reason: TerminalRetryReason) -> RouteDeadLetterOutcome {
    match reason {
        TerminalRetryReason::Fatal => RouteDeadLetterOutcome::Fatal,
        TerminalRetryReason::Exhausted => RouteDeadLetterOutcome::Exhausted,
    }
}

fn running_worker_id(task: &PgTask<Value>) -> Option<String> {
    let worker = task.parts.data.get::<WorkerContext>()?;
    let lock_by = task.parts.ctx.lock_by().as_deref()?;
    (worker.name() == lock_by).then(|| lock_by.to_owned())
}

fn current_attempt(attempt: usize) -> NonZeroU32 {
    match u32::try_from(attempt) {
        Ok(0) => {
            error!("Apalis delivered a task with a zero attempt count; using one");
            NonZeroU32::MIN
        }
        Ok(attempt) => NonZeroU32::new(attempt).unwrap_or(NonZeroU32::MIN),
        Err(_) => {
            error!(attempt, "Apalis attempt count exceeds u32; saturating");
            NonZeroU32::new(u32::MAX).unwrap_or(NonZeroU32::MIN)
        }
    }
}

fn random_jitter() -> JitterSample {
    let sample = rand::thread_rng().gen_range(0.0..=1.0);
    JitterSample::new(sample).unwrap_or_else(|_| JitterSample::midpoint())
}

#[derive(Debug)]
struct RetryAckError(String);

impl std::fmt::Display for RetryAckError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for RetryAckError {}

fn error_kind_label(kind: JobErrorKind) -> &'static str {
    match kind {
        JobErrorKind::Retryable => "retryable",
        JobErrorKind::Fatal => "fatal",
    }
}

fn decision_label(decision: RetryWrapperDecision) -> &'static str {
    match decision {
        RetryWrapperDecision::Schedule { .. } => "scheduled",
        RetryWrapperDecision::Terminal { .. } => "dead_lettered",
    }
}

#[cfg(test)]
mod tests;
