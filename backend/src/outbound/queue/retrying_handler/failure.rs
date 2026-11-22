//! Failure classification and persistence for the retrying Apalis service.

use std::num::NonZeroU32;
use std::time::Duration;

use apalis_core::{error::BoxDynError, worker::context::WorkerContext};
use apalis_postgres::PgTask;
use serde_json::Value;
use tracing::{error, info, warn};

use super::JitterSource;
use super::decision::{RetryDecisionContext, RetryWrapperDecision, decide_retry_wrapper_action};
use crate::domain::jobs::retry::{JobErrorKind, JobFailure, JobRetryPolicy};
use crate::domain::ports::{RouteQueueMetrics, RouteRetryOutcome};
use crate::outbound::queue::RetryPersistence;
use crate::outbound::queue::RetryScheduleUpdate;

mod clamp;
mod terminal;

use terminal::{TerminalRetryPlan, persist_terminal_failure};

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
        .schedule_retry(RetryScheduleUpdate {
            task_id: identity.task_id,
            worker_id: identity.worker_id,
            attempt: context.attempt,
            delay,
        })
        .await
    {
        Ok(true) => {
            tracing::Span::current().record("outcome", "scheduled");
            context.metrics.observe_retry(RouteRetryOutcome::Scheduled);
            // A scheduled retry is the wrapper's normal degraded path, not a
            // fault: the job will run again, so this is an informational
            // outcome and warnings stay reserved for refused or failed writes.
            info!(
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

/// Report whether a delivery has already spent its job's stored attempt cap.
///
/// Apalis stores `attempts` as the number of deliveries a row has received and
/// increments the in-memory attempt on the first poll, so a legal delivery
/// carries an attempt at or below the row's `max_attempts`. A larger attempt
/// means the row was made fetchable without its cap agreeing: orphan recovery
/// returns a row to `Pending` and increments `attempts`, and the fetch
/// predicate accepts `Pending` rows without consulting `max_attempts`. That is
/// the window in which a job classified as terminal between its terminal write
/// and its acknowledgement becomes deliverable again.
///
/// The comparison is deliberately not
/// [`JobAttemptBudget::permits`](crate::domain::jobs::retry::JobAttemptBudget::permits): that
/// answers whether another retry may follow, so it refuses the legal final
/// delivery at the cap. This question is whether the delivery may run at all,
/// which the cap itself answers.
///
/// # Examples
///
/// ```text
/// assert!(!is_spent_attempt(attempt(5), 5), "the final permitted attempt runs");
/// assert!(is_spent_attempt(attempt(6), 5), "a recovered row past its cap does not");
/// ```
pub(super) fn is_spent_attempt(attempt: NonZeroU32, context_max_attempts: u32) -> bool {
    attempt.get() > context_max_attempts
}

/// Refuse one delivery whose stored attempt cap is already spent.
///
/// The refusal returns an error without invoking the handler, so the
/// acknowledgement that follows records a delivery that never ran. Logging the
/// values keeps the case visible: an ordinary fetch cannot produce it, so a
/// line here means orphan recovery returned a row whose cap had already
/// retired it.
pub(super) fn refuse_spent_attempt(
    attempt: NonZeroU32,
    context_max_attempts: u32,
    queue_name: &str,
) -> BoxDynError {
    warn!(
        queue = queue_name,
        attempt = attempt.get(),
        max_attempts = context_max_attempts,
        decision = "spent_attempt_refused",
        "delivery exceeds the job's stored attempt cap; skipping the handler"
    );
    Box::new(SpentAttemptError {
        attempt,
        max_attempts: context_max_attempts,
    })
}

/// A delivery whose attempt count already exceeds its job's stored cap.
#[derive(Debug)]
struct SpentAttemptError {
    attempt: NonZeroU32,
    max_attempts: u32,
}

impl std::fmt::Display for SpentAttemptError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "delivery attempt {} exceeds the stored attempt cap {}",
            self.attempt, self.max_attempts
        )
    }
}

impl std::error::Error for SpentAttemptError {}

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
pub(super) fn error_kind_label(kind: JobErrorKind) -> &'static str {
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
