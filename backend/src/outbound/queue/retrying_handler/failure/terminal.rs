//! Persistence and recording for a terminal retry decision.
//!
//! A terminal decision ends the job's retry budget. The wrapper asks for one
//! guarded write that stores the dead-letter reason, records the dead-letter
//! metric only when that write applies, and lowers the attempt cap when the
//! write itself fails so Apalis does not run the retired job again.

use tracing::warn;

use super::clamp::clamp_terminal_cap;
use super::{RetryFailureContext, RetryJobIdentity, error_kind_label};
use crate::domain::jobs::retry::JobErrorKind;
use crate::domain::ports::RouteDeadLetterOutcome;
use crate::outbound::queue::TerminalRetryUpdate;
use crate::outbound::queue::postgres_retry_scheduler::TerminalRetryReason;

/// The terminal decision a failed delivery asked the wrapper to persist.
#[derive(Clone, Copy)]
pub(super) struct TerminalRetryPlan {
    pub(super) reason: TerminalRetryReason,
    pub(super) shrink_limit: bool,
}

/// Record a terminal failure's dead-letter classification.
///
/// Only an applied write counts as a dead-letter outcome. A lost ownership
/// guard or a failed write stores no dead-letter reason, so it counts as
/// neither a retry nor a dead letter; it increments the terminal-write-failure
/// counter and logs a warning instead. A failed write is then followed by
/// [`clamp_terminal_cap`], because the handler still returns an error and must
/// not let Apalis run the retired job again.
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
pub(super) async fn persist_terminal_failure(
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
        Err(error) => {
            record_refused_terminal_write(context, identity, write, Some(&error));
            clamp_terminal_cap(context, identity).await;
        }
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
/// running row; neither stores a dead-letter reason, and both count the same
/// way.
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
