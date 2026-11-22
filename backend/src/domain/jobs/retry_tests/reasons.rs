//! Terminal-reason tests for the domain retry decision.
//!
//! The adapter stores a dead-letter label, but it must not choose the label:
//! the domain decision names the terminal reason, and these tests pin that
//! choice for each failure classification.

use std::num::NonZeroU32;

use pretty_assertions::assert_eq;
use rstest::rstest;

use super::{policy, valid_policy_config};
use crate::domain::jobs::retry::{
    BoundedAttempt, JitterSample, JobAttemptBudget, JobErrorKind, RetryDecision,
};
use crate::domain::ports::DeadReason;

/// Scenario: a failure the handler classified as permanent.
///
/// Invariant: the domain decision, not the adapter, records that the job is
/// terminal because the failure was fatal, even when attempts remain.
#[rstest]
#[case::below_every_limit(JobAttemptBudget::unlimited(), 1)]
#[case::at_the_policy_ceiling(JobAttemptBudget::unlimited(), 3)]
#[case::with_a_spent_job_budget(JobAttemptBudget::from_max_attempts(1), 1)]
fn fatal_failures_dead_letter_with_the_fatal_reason(
    #[case] budget: JobAttemptBudget,
    #[case] attempt: u32,
) {
    let policy = policy(valid_policy_config()).expect("valid policy should build");

    let decision = policy.decide(
        BoundedAttempt::new(
            NonZeroU32::new(attempt).expect("case attempt is positive"),
            budget,
        ),
        JobErrorKind::Fatal,
        JitterSample::midpoint(),
    );

    assert_eq!(
        decision,
        RetryDecision::DeadLetter {
            reason: DeadReason::Fatal,
        },
        "a fatal failure at attempt {attempt} must record the fatal terminal reason"
    );
}

/// Scenario: a retryable failure with no attempt left under either limit.
///
/// Invariant: reaching a limit yields the exhausted reason, whether the policy
/// ceiling or the job's stored budget is the limit that binds.
#[rstest]
#[case::at_the_policy_ceiling(JobAttemptBudget::unlimited(), 3)]
#[case::above_the_policy_ceiling(JobAttemptBudget::unlimited(), 4)]
#[case::at_the_job_budget(JobAttemptBudget::from_max_attempts(2), 2)]
fn exhausted_retryable_failures_dead_letter_with_the_exhausted_reason(
    #[case] budget: JobAttemptBudget,
    #[case] attempt: u32,
) {
    let policy = policy(valid_policy_config()).expect("valid policy should build");

    let decision = policy.decide(
        BoundedAttempt::new(
            NonZeroU32::new(attempt).expect("case attempt is positive"),
            budget,
        ),
        JobErrorKind::Retryable,
        JitterSample::midpoint(),
    );

    assert_eq!(
        decision,
        RetryDecision::DeadLetter {
            reason: DeadReason::Exhausted,
        },
        "a retryable failure at attempt {attempt} with budget {budget:?} must record exhaustion"
    );
}
