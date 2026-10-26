//! Pure retry decisions combining the domain and Apalis attempt limits.

use std::num::NonZeroU32;
use std::time::Duration;

use crate::domain::jobs::retry::{JitterSample, JobErrorKind, JobRetryPolicy, RetryDecision};
use crate::outbound::queue::postgres_retry_scheduler::TerminalRetryReason;

/// Outcome selected by the adapter's pure comparison of both attempt budgets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RetryWrapperDecision {
    /// Persist the next `run_at` and let Apalis acknowledge this failure.
    Schedule { delay: Duration },
    /// Persist a terminal reason, optionally lowering the stored attempt cap.
    Terminal {
        reason: TerminalRetryReason,
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
/// task. A retry is scheduled only while both that limit and the queue policy
/// leave another attempt.
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
    let shrink_limit = context.attempt.get() < context.context_max_attempts;
    match policy.decide(context.attempt, context.kind, context.jitter) {
        RetryDecision::Retry { delay } if context.attempt.get() < context.context_max_attempts => {
            RetryWrapperDecision::Schedule { delay }
        }
        RetryDecision::Retry { .. } => terminal_decision(context.kind, shrink_limit),
        RetryDecision::DeadLetter => terminal_decision(context.kind, shrink_limit),
    }
}

fn terminal_decision(kind: JobErrorKind, shrink_limit: bool) -> RetryWrapperDecision {
    let reason = match kind {
        JobErrorKind::Fatal => TerminalRetryReason::Fatal,
        JobErrorKind::Retryable => TerminalRetryReason::Exhausted,
    };
    RetryWrapperDecision::Terminal {
        reason,
        shrink_limit,
    }
}
