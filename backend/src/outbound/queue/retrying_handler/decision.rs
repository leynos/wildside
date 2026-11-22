//! Pure retry decisions combining the domain and Apalis attempt limits.

use std::num::NonZeroU32;
use std::time::Duration;

use crate::domain::jobs::retry::{
    BoundedAttempt, JitterSample, JobAttemptBudget, JobErrorKind, JobRetryPolicy, RetryDecision,
};
use crate::domain::ports::DeadReason;

/// Outcome selected by the adapter's pure comparison of both attempt budgets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RetryWrapperDecision {
    /// Persist the next `run_at` and let Apalis acknowledge this failure.
    Schedule { delay: Duration },
    /// Persist a terminal reason, optionally lowering the stored attempt cap.
    Terminal {
        reason: DeadReason,
        shrink_limit: bool,
    },
}

#[derive(Clone, Copy)]
pub(super) struct RetryDecisionContext {
    pub(super) attempt: NonZeroU32,
    pub(super) context_max_attempts: u32,
    pub(super) kind: JobErrorKind,
    pub(super) jitter: JitterSample,
}

/// Select the durable action for a classified handler failure.
///
/// The context limit reflects the `max_attempts` held by the fetched Apalis
/// task. It becomes the job's [`JobAttemptBudget`], so the domain weighs that
/// limit and the queue policy together, and a retry is scheduled only while
/// both leave another attempt.
///
/// # Examples
///
/// ```text
/// let decision = decide_retry_wrapper_action(
///     &queue_policy,
///     RetryDecisionContext {
///         attempt: current_attempt,
///         context_max_attempts: task_context.max_attempts(),
///         kind: JobErrorKind::Retryable,
///         jitter: jitter_sample,
///     },
/// );
/// match decision {
///     RetryWrapperDecision::Schedule { delay } => persist_run_at(delay).await?,
///     RetryWrapperDecision::Terminal { reason, shrink_limit } => {
///         persist_dead_reason(reason, shrink_limit).await?;
///     }
/// }
/// ```
pub(super) fn decide_retry_wrapper_action(
    policy: &JobRetryPolicy,
    context: RetryDecisionContext,
) -> RetryWrapperDecision {
    let delivery = BoundedAttempt::new(
        context.attempt,
        JobAttemptBudget::from_max_attempts(context.context_max_attempts),
    );
    // Apalis acknowledges a failed delivery by storing this attempt count, and
    // it keeps fetching a failed row while that count is below the row's cap.
    // The cap therefore needs lowering only when this attempt is strictly
    // below the cap the fetched task carried. `is_permitted` compares exactly
    // those two values; the terminal reason itself comes from the domain
    // decision below, so the flag and the reason stay independent.
    let shrink_limit = delivery.is_permitted();
    match policy.decide(delivery, context.kind, context.jitter) {
        RetryDecision::Retry { delay } => RetryWrapperDecision::Schedule { delay },
        RetryDecision::DeadLetter { reason } => RetryWrapperDecision::Terminal {
            reason,
            shrink_limit,
        },
    }
}
