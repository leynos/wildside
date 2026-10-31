//! Unit tests for retry wrapper decisions and scheduling context.

use std::num::NonZeroU32;
use std::time::Duration;

use rstest::rstest;

use super::{RetryDecisionContext, RetryWrapperDecision, decide_retry_wrapper_action};
use crate::domain::jobs::retry::{
    JitterSample, JobErrorKind, JobRetryPolicy, JobRetryPolicyConfig,
};
use crate::outbound::queue::postgres_retry_scheduler::TerminalRetryReason;

#[derive(Debug)]
struct RetryWrapperCase {
    kind: JobErrorKind,
    attempt: u32,
    policy_limit: u32,
    context_limit: u32,
    expected: RetryWrapperDecision,
}

#[rstest]
#[case::retry_below_both_limits(RetryWrapperCase {
    kind: JobErrorKind::Retryable,
    attempt: 1,
    policy_limit: 3,
    context_limit: 5,
    expected: RetryWrapperDecision::Schedule { delay: Duration::from_millis(10) },
})]
#[case::retry_uses_attempt_based_delay(RetryWrapperCase {
    kind: JobErrorKind::Retryable,
    attempt: 2,
    policy_limit: 3,
    context_limit: 5,
    expected: RetryWrapperDecision::Schedule { delay: Duration::from_millis(20) },
})]
#[case::policy_limit_exhausted_before_context_limit(RetryWrapperCase {
    kind: JobErrorKind::Retryable,
    attempt: 3,
    policy_limit: 3,
    context_limit: 25,
    expected: RetryWrapperDecision::Terminal {
        reason: TerminalRetryReason::Exhausted,
        shrink_limit: true,
    },
})]
#[case::fatal_failure_with_spare_apalis_attempts(RetryWrapperCase {
    kind: JobErrorKind::Fatal,
    attempt: 1,
    policy_limit: 3,
    context_limit: 5,
    expected: RetryWrapperDecision::Terminal {
        reason: TerminalRetryReason::Fatal,
        shrink_limit: true,
    },
})]
#[case::fatal_at_apalis_limit_needs_no_shrink(RetryWrapperCase {
    kind: JobErrorKind::Fatal,
    attempt: 3,
    policy_limit: 3,
    context_limit: 3,
    expected: RetryWrapperDecision::Terminal {
        reason: TerminalRetryReason::Fatal,
        shrink_limit: false,
    },
})]
#[case::context_limit_exhausted_before_policy_limit(RetryWrapperCase {
    kind: JobErrorKind::Retryable,
    attempt: 1,
    policy_limit: 5,
    context_limit: 1,
    expected: RetryWrapperDecision::Terminal {
        reason: TerminalRetryReason::Exhausted,
        shrink_limit: false,
    },
})]
#[case::context_limit_is_terminal_at_current_attempt(RetryWrapperCase {
    kind: JobErrorKind::Retryable,
    attempt: 2,
    policy_limit: 5,
    context_limit: 2,
    expected: RetryWrapperDecision::Terminal {
        reason: TerminalRetryReason::Exhausted,
        shrink_limit: false,
    },
})]
#[case::fatal_at_policy_limit_still_uses_fatal_reason(RetryWrapperCase {
    kind: JobErrorKind::Fatal,
    attempt: 3,
    policy_limit: 3,
    context_limit: 25,
    expected: RetryWrapperDecision::Terminal {
        reason: TerminalRetryReason::Fatal,
        shrink_limit: true,
    },
})]
fn retry_wrapper_maps_policy_and_context_budget(#[case] case: RetryWrapperCase) {
    let policy = policy(case.policy_limit);
    let decision = decide_retry_wrapper_action(
        &policy,
        RetryDecisionContext {
            attempt: NonZeroU32::new(case.attempt).expect("positive attempt"),
            context_max_attempts: case.context_limit,
            kind: case.kind,
            jitter: JitterSample::midpoint(),
        },
    );

    assert_eq!(decision, case.expected);
}

#[test]
fn retry_wrapper_decision_table_covers_both_attempt_limits() {
    const CURRENT_ATTEMPT: u32 = 2;
    let relations = [
        AttemptRelation::Below,
        AttemptRelation::At,
        AttemptRelation::Above,
    ];

    for kind in [JobErrorKind::Retryable, JobErrorKind::Fatal] {
        for policy_relation in relations {
            for context_relation in relations {
                let policy_limit = policy_relation.budget_for(CURRENT_ATTEMPT);
                let context_limit = context_relation.budget_for(CURRENT_ATTEMPT);
                let policy = policy(policy_limit);
                let decision = decide_retry_wrapper_action(
                    &policy,
                    RetryDecisionContext {
                        attempt: NonZeroU32::new(CURRENT_ATTEMPT).expect("positive attempt"),
                        context_max_attempts: context_limit,
                        kind,
                        jitter: JitterSample::midpoint(),
                    },
                );

                let has_policy_attempt = policy_limit > CURRENT_ATTEMPT;
                let has_context_attempt = context_limit > CURRENT_ATTEMPT;
                let expected =
                    expected_retry_wrapper_decision(kind, has_policy_attempt, has_context_attempt);
                assert_eq!(
                    decision, expected,
                    "kind={kind:?}, policy={policy_relation:?}, context={context_relation:?}"
                );
            }
        }
    }
}

fn expected_retry_wrapper_decision(
    kind: JobErrorKind,
    has_policy_attempt: bool,
    has_context_attempt: bool,
) -> RetryWrapperDecision {
    if kind == JobErrorKind::Fatal {
        return RetryWrapperDecision::Terminal {
            reason: TerminalRetryReason::Fatal,
            shrink_limit: has_context_attempt,
        };
    }
    if has_policy_attempt && has_context_attempt {
        return RetryWrapperDecision::Schedule {
            delay: Duration::from_millis(20),
        };
    }
    RetryWrapperDecision::Terminal {
        reason: TerminalRetryReason::Exhausted,
        shrink_limit: has_context_attempt,
    }
}

#[derive(Clone, Copy, Debug)]
enum AttemptRelation {
    Below,
    At,
    Above,
}

impl AttemptRelation {
    fn budget_for(self, attempt: u32) -> u32 {
        match self {
            Self::Below => attempt + 1,
            Self::At => attempt,
            Self::Above => attempt - 1,
        }
    }
}

fn policy(max_attempts: u32) -> JobRetryPolicy {
    JobRetryPolicy::new(JobRetryPolicyConfig {
        max_attempts: NonZeroU32::new(max_attempts).expect("positive attempt limit"),
        base_delay: Duration::from_millis(10),
        multiplier: 2.0,
        max_delay: Duration::from_millis(80),
        jitter_fraction: 0.0,
    })
    .expect("valid test policy")
}
