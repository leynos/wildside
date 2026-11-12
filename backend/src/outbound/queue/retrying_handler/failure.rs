//! Failure classification and persistence for the retrying Apalis service.

use std::num::NonZeroU32;
use std::time::Duration;

use apalis_core::{error::BoxDynError, worker::context::WorkerContext};
use apalis_postgres::PgTask;
use serde_json::Value;
use tracing::{error, warn};

use super::JitterSource;
use super::decision::{RetryDecisionContext, RetryWrapperDecision, decide_retry_wrapper_action};
use crate::domain::jobs::retry::{JobErrorKind, JobFailure, JobRetryPolicy};
use crate::domain::ports::{RouteDeadLetterOutcome, RouteQueueMetrics, RouteRetryOutcome};
use crate::outbound::queue::RetryPersistence;
use crate::outbound::queue::TerminalRetryUpdate;
use crate::outbound::queue::postgres_retry_scheduler::TerminalRetryReason;

/// Everything one failed delivery needs to classify and persist its outcome.
pub(super) struct RetryFailureContext<'a> {
    pub(super) attempt: NonZeroU32,
    pub(super) context_max_attempts: u32,
    pub(super) policy: &'a JobRetryPolicy,
    pub(super) scheduler: &'a dyn RetryPersistence,
    pub(super) metrics: &'a dyn RouteQueueMetrics,
    pub(super) jitter: &'a dyn JitterSource,
    pub(super) queue_name: &'a str,
    pub(super) task_id: Option<&'a str>,
    pub(super) worker_id: Option<&'a str>,
}

/// Classify one failed delivery and persist its retry or terminal outcome.
///
/// The wrapper always returns an error so Apalis keeps the task failed. It
/// writes nothing when it cannot prove the current worker owns the row, and it
/// records the failure metric for a retryable decision in that case.
#[tracing::instrument(
    skip_all,
    fields(
        queue = context.queue_name,
        attempt = context.attempt.get(),
        error_kind = error_kind_label(failure.kind()),
        outcome = tracing::field::Empty,
    )
)]
pub(super) async fn handle_failure(
    failure: JobFailure,
    context: &RetryFailureContext<'_>,
) -> Result<(), BoxDynError> {
    let kind = failure.kind();
    let jitter = context.jitter.sample();
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
            tracing::Span::current().record("outcome", "refused");
            record_missing_retry_metadata(context, kind, decision);
            return Err(Box::new(RetryAckError(error_message)));
        }
    };
    tracing::Span::current().record("outcome", decision_label(decision));

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

#[derive(Clone, Copy)]
struct RetryJobIdentity<'a> {
    task_id: &'a str,
    worker_id: &'a str,
}

#[derive(Clone, Copy)]
struct TerminalRetryPlan {
    reason: TerminalRetryReason,
    shrink_limit: bool,
}

/// Record that a failure could not be persisted because its identity is absent.
///
/// A retryable decision counts as a scheduling failure, because the retry the
/// policy asked for will not happen. A terminal decision records no dead-letter
/// outcome: the row was never classified, so claiming one would overstate what
/// the wrapper achieved.
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

/// Persist the next `run_at` for a retryable failure and record its outcome.
///
/// The scheduler's ownership guard decides whether the write applied. A
/// `false` result means another worker now owns the row, and an error means the
/// write failed; both count as a scheduling failure.
#[tracing::instrument(
    skip_all,
    fields(
        queue = context.queue_name,
        task_id = identity.task_id,
        worker_id = identity.worker_id,
        attempt = context.attempt.get(),
        delay_micros = %delay.as_micros(),
        outcome = tracing::field::Empty,
    )
)]
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
            tracing::Span::current().record("outcome", "scheduled");
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
            tracing::Span::current().record("outcome", "schedule_failure");
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
            tracing::Span::current().record("outcome", "schedule_failure");
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

/// Record a terminal failure's dead-letter classification.
///
/// Only an applied write counts as a dead-letter outcome. A lost ownership
/// guard or a failed write changes no stored state, so it counts as neither a
/// retry nor a dead letter; it increments the terminal-write-failure counter
/// and logs a warning instead.
#[tracing::instrument(
    skip_all,
    fields(
        queue = context.queue_name,
        task_id = identity.task_id,
        worker_id = identity.worker_id,
        attempt = context.attempt.get(),
        reason = plan.reason.as_label(),
        outcome = tracing::field::Empty,
    )
)]
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
    let write = TerminalWrite {
        kind,
        reason,
        shrink_limit,
    };
    match result {
        Ok(true) => record_dead_letter(context, identity, write),
        Ok(false) => record_refused_terminal_write(context, identity, write, None),
        Err(error) => record_refused_terminal_write(context, identity, write, Some(&error)),
    }
}

/// The decision a terminal write was asked to persist.
#[derive(Clone, Copy)]
struct TerminalWrite {
    kind: JobErrorKind,
    reason: TerminalRetryReason,
    shrink_limit: bool,
}

/// Record a terminal write that applied and stored its dead-letter reason.
fn record_dead_letter(
    context: &RetryFailureContext<'_>,
    identity: RetryJobIdentity<'_>,
    write: TerminalWrite,
) {
    let TerminalWrite {
        kind,
        reason,
        shrink_limit,
    } = write;
    tracing::Span::current().record("outcome", "dead_lettered");
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

/// Record a terminal write the ownership guard or the database refused.
///
/// `error` distinguishes a write that failed from one whose guard matched no
/// running row; both leave the stored row untouched and count the same way.
fn record_refused_terminal_write(
    context: &RetryFailureContext<'_>,
    identity: RetryJobIdentity<'_>,
    write: TerminalWrite,
    error: Option<&sqlx::Error>,
) {
    let TerminalWrite { kind, reason, .. } = write;
    tracing::Span::current().record("outcome", "terminal_write_failure");
    context
        .metrics
        .observe_terminal_write_failure(dead_letter_outcome(reason));
    if let Some(error) = error {
        warn!(
            queue = context.queue_name,
            task_id = identity.task_id,
            worker_id = identity.worker_id,
            attempt = context.attempt.get(),
            error_kind = error_kind_label(kind),
            decision = "terminal_write_failure",
            reason = reason.as_label(),
            error = %error,
            "job terminal update failed"
        );
    } else {
        warn!(
            queue = context.queue_name,
            task_id = identity.task_id,
            worker_id = identity.worker_id,
            attempt = context.attempt.get(),
            error_kind = error_kind_label(kind),
            decision = "terminal_write_failure",
            reason = reason.as_label(),
            "job terminal update no longer owns a running row"
        );
    }
}

/// Map a terminal reason to the metric label it increments.
fn dead_letter_outcome(reason: TerminalRetryReason) -> RouteDeadLetterOutcome {
    match reason {
        TerminalRetryReason::Fatal => RouteDeadLetterOutcome::Fatal,
        TerminalRetryReason::Exhausted => RouteDeadLetterOutcome::Exhausted,
    }
}

/// Return the worker that owns the row, if the delivery proves ownership.
///
/// Apalis records the locking worker on the task context and the running
/// worker in the task data. The wrapper persists only when both are present
/// and name the same worker, so a stale or foreign delivery cannot overwrite
/// another worker's row.
pub(super) fn running_worker_id(task: &PgTask<Value>) -> Option<String> {
    let worker = task.parts.data.get::<WorkerContext>()?;
    let lock_by = task.parts.ctx.lock_by().as_deref()?;
    (worker.name() == lock_by).then(|| lock_by.to_owned())
}

/// Convert Apalis's attempt counter into the domain's non-zero attempt.
///
/// A zero or out-of-range counter is a delivery defect rather than a retry
/// signal, so it is logged and clamped instead of failing the delivery.
pub(super) fn current_attempt(attempt: usize) -> NonZeroU32 {
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

#[derive(Debug)]
struct RetryAckError(String);

impl std::fmt::Display for RetryAckError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for RetryAckError {}

/// Return the stable log label for a failure kind.
fn error_kind_label(kind: JobErrorKind) -> &'static str {
    match kind {
        JobErrorKind::Retryable => "retryable",
        JobErrorKind::Fatal => "fatal",
    }
}

/// Return the stable log label for a wrapper decision.
fn decision_label(decision: RetryWrapperDecision) -> &'static str {
    match decision {
        RetryWrapperDecision::Schedule { .. } => "scheduled",
        RetryWrapperDecision::Terminal { .. } => "dead_lettered",
    }
}
