//! Unit tests for domain retry policy and failure classification.

use std::num::NonZeroU32;
use std::time::Duration;

use pretty_assertions::assert_eq;
use rstest::rstest;

use super::retry::{
    BoundedAttempt, JitterSample, JitterSampleError, JobAttemptBudget, JobErrorKind, JobFailure,
    JobRetryPolicy, JobRetryPolicyBuildError, JobRetryPolicyConfig, MAX_BACKOFF_DELAY,
    MAX_JOB_FAILURE_MESSAGE_BYTES, RetryDecision,
};
use crate::domain::ports::{DeadReason, EnrichmentSourceError};

mod properties;
mod reasons;

fn policy(config: JobRetryPolicyConfig) -> Result<JobRetryPolicy, JobRetryPolicyBuildError> {
    JobRetryPolicy::new(config)
}

fn valid_policy_config() -> JobRetryPolicyConfig {
    JobRetryPolicyConfig {
        max_attempts: NonZeroU32::new(3).expect("positive attempt limit"),
        base_delay: Duration::from_secs(1),
        multiplier: 2.0,
        max_delay: Duration::from_secs(10),
        jitter_fraction: 0.0,
    }
}

fn policy_config_with(
    base_delay: Duration,
    multiplier: f64,
    max_delay: Duration,
    jitter_fraction: f64,
) -> JobRetryPolicyConfig {
    JobRetryPolicyConfig {
        base_delay,
        multiplier,
        max_delay,
        jitter_fraction,
        ..valid_policy_config()
    }
}

#[rstest]
#[case::zero_base_delay(
    policy_config_with(Duration::ZERO, 2.0, Duration::from_secs(10), 0.0),
    JobRetryPolicyBuildError::ZeroBaseDelay
)]
#[case::base_delay_precedes_other_errors(
    policy_config_with(Duration::ZERO, 0.0, Duration::ZERO, -0.1),
    JobRetryPolicyBuildError::ZeroBaseDelay
)]
#[case::multiplier_precedes_maximum_and_jitter_errors(
    policy_config_with(Duration::from_secs(1), 0.0, Duration::ZERO, -0.1),
    JobRetryPolicyBuildError::InvalidMultiplier
)]
#[case::maximum_precedes_jitter_errors(
    policy_config_with(Duration::from_secs(1), 1.0, Duration::ZERO, -0.1),
    JobRetryPolicyBuildError::ZeroMaxDelay
)]
#[case::zero_multiplier(
    policy_config_with(Duration::from_secs(1), 0.0, Duration::from_secs(10), 0.0),
    JobRetryPolicyBuildError::InvalidMultiplier
)]
#[case::nan_multiplier(
    policy_config_with(Duration::from_secs(1), f64::NAN, Duration::from_secs(10), 0.0),
    JobRetryPolicyBuildError::InvalidMultiplier
)]
#[case::negative_multiplier(
    policy_config_with(Duration::from_secs(1), -1.0, Duration::from_secs(10), 0.0),
    JobRetryPolicyBuildError::InvalidMultiplier,
)]
#[case::infinite_multiplier(
    policy_config_with(Duration::from_secs(1), f64::INFINITY, Duration::from_secs(10), 0.0),
    JobRetryPolicyBuildError::InvalidMultiplier
)]
#[case::zero_max_delay(
    policy_config_with(Duration::from_secs(1), 1.0, Duration::ZERO, 0.0),
    JobRetryPolicyBuildError::ZeroMaxDelay
)]
#[case::max_delay_too_large(
    policy_config_with(Duration::from_secs(1), 1.0, Duration::from_secs(3601), 0.0),
    JobRetryPolicyBuildError::MaxDelayExceedsMaximum
)]
#[case::negative_jitter(
    policy_config_with(Duration::from_secs(1), 1.0, Duration::from_secs(10), -0.1),
    JobRetryPolicyBuildError::InvalidJitterFraction,
)]
#[case::nan_jitter(
    policy_config_with(Duration::from_secs(1), 1.0, Duration::from_secs(10), f64::NAN),
    JobRetryPolicyBuildError::InvalidJitterFraction
)]
#[case::jitter_above_one(
    policy_config_with(Duration::from_secs(1), 1.0, Duration::from_secs(10), 1.1),
    JobRetryPolicyBuildError::InvalidJitterFraction
)]
#[case::jittered_delay_over_ceiling(
    policy_config_with(Duration::from_secs(1), 1.0, Duration::from_secs(1801), 1.0),
    JobRetryPolicyBuildError::JitteredDelayExceedsMaximum
)]
fn constructor_rejects_invalid_configuration(
    #[case] config: JobRetryPolicyConfig,
    #[case] expected: JobRetryPolicyBuildError,
) {
    let error = policy(config).expect_err("invalid policy parameters must be rejected");

    assert_eq!(error, expected);
}

#[rstest]
#[case::below_ceiling(Duration::from_secs(1799), None)]
#[case::exactly_at_ceiling(Duration::from_secs(1800), None)]
#[case::above_ceiling(
    Duration::from_secs(1801),
    Some(JobRetryPolicyBuildError::JitteredDelayExceedsMaximum)
)]
fn constructor_checks_the_jittered_delay_ceiling(
    #[case] max_delay: Duration,
    #[case] expected: Option<JobRetryPolicyBuildError>,
) {
    let result = policy(JobRetryPolicyConfig {
        multiplier: 1.0,
        max_delay,
        jitter_fraction: 1.0,
        ..valid_policy_config()
    });

    assert_eq!(
        result.err(),
        expected,
        "only a jittered delay over the ceiling may be refused"
    );
}

#[rstest]
#[case::low_endpoint(0.0, None)]
#[case::midpoint(0.5, None)]
#[case::high_endpoint(1.0, None)]
#[case::negative(-f64::EPSILON, Some(JitterSampleError::OutOfRange))]
#[case::above_one(1.0 + f64::EPSILON, Some(JitterSampleError::OutOfRange))]
#[case::nan(f64::NAN, Some(JitterSampleError::NotFinite))]
#[case::positive_infinity(f64::INFINITY, Some(JitterSampleError::NotFinite))]
#[case::negative_infinity(f64::NEG_INFINITY, Some(JitterSampleError::NotFinite))]
fn jitter_sample_isolates_the_closed_unit_interval(
    #[case] sample: f64,
    #[case] expected: Option<JitterSampleError>,
) {
    assert_eq!(
        JitterSample::new(sample).err(),
        expected,
        "only the closed unit interval is a valid jitter sample"
    );
}

#[rstest]
#[case::low_endpoint(0.0, Duration::from_millis(500))]
#[case::midpoint(0.5, Duration::from_secs(1))]
#[case::high_endpoint(1.0, Duration::from_millis(1500))]
fn jitter_maps_linearly_around_the_nominal_delay(#[case] sample: f64, #[case] expected: Duration) {
    let policy = policy(JobRetryPolicyConfig {
        jitter_fraction: 0.5,
        ..valid_policy_config()
    })
    .expect("valid policy should build");
    let jitter = JitterSample::new(sample).expect("case sample should be valid");

    assert_eq!(
        policy.backoff_delay(NonZeroU32::new(1).expect("positive attempt"), jitter),
        expected
    );
}

/// The once-retried delay the valid policy reaches on its second attempt.
fn retried_delay() -> RetryDecision {
    RetryDecision::Retry {
        delay: Duration::from_secs(2),
    }
}

/// The terminal decision a fatal failure selects.
fn fatal_dead_letter() -> RetryDecision {
    RetryDecision::DeadLetter {
        reason: DeadReason::Fatal,
    }
}

/// The terminal decision an exhausted retryable failure selects.
fn exhausted_dead_letter() -> RetryDecision {
    RetryDecision::DeadLetter {
        reason: DeadReason::Exhausted,
    }
}

#[rstest]
#[case::fatal_below_policy_bound(
    JobErrorKind::Fatal,
    JobAttemptBudget::unlimited(),
    2,
    fatal_dead_letter()
)]
#[case::fatal_at_policy_bound(
    JobErrorKind::Fatal,
    JobAttemptBudget::unlimited(),
    3,
    fatal_dead_letter()
)]
#[case::fatal_above_policy_bound(
    JobErrorKind::Fatal,
    JobAttemptBudget::unlimited(),
    4,
    fatal_dead_letter()
)]
#[case::retryable_below_policy_bound(
    JobErrorKind::Retryable,
    JobAttemptBudget::unlimited(),
    2,
    retried_delay()
)]
#[case::retryable_at_policy_bound(
    JobErrorKind::Retryable,
    JobAttemptBudget::unlimited(),
    3,
    exhausted_dead_letter()
)]
#[case::retryable_above_policy_bound(
    JobErrorKind::Retryable,
    JobAttemptBudget::unlimited(),
    4,
    exhausted_dead_letter()
)]
#[case::policy_ceiling_binds(
    JobErrorKind::Retryable,
    JobAttemptBudget::from_max_attempts(5),
    3,
    exhausted_dead_letter()
)]
#[case::job_budget_binds(
    JobErrorKind::Retryable,
    JobAttemptBudget::from_max_attempts(2),
    2,
    exhausted_dead_letter()
)]
#[case::both_leave_another_attempt(
    JobErrorKind::Retryable,
    JobAttemptBudget::from_max_attempts(5),
    2,
    retried_delay()
)]
#[case::zero_budget_forbids_the_first_attempt(
    JobErrorKind::Retryable,
    JobAttemptBudget::from_max_attempts(0),
    1,
    exhausted_dead_letter()
)]
fn decision_table_is_total_and_honours_every_lower_bound(
    #[case] kind: JobErrorKind,
    #[case] budget: JobAttemptBudget,
    #[case] attempt: u32,
    #[case] expected: RetryDecision,
) {
    let policy = policy(valid_policy_config()).expect("valid policy should build");
    let jitter = JitterSample::new(0.5).expect("midpoint is valid");
    let delivery = BoundedAttempt::new(
        NonZeroU32::new(attempt).expect("case attempt is positive"),
        budget,
    );

    assert_eq!(
        policy.decide(delivery, kind, jitter),
        expected,
        "kind={kind:?}, budget={budget:?}, attempt={attempt} must select {expected:?}"
    );
}

#[rstest]
#[case::transport(EnrichmentSourceError::transport("temporary"), JobErrorKind::Retryable)]
#[case::timeout(EnrichmentSourceError::timeout("slow"), JobErrorKind::Retryable)]
#[case::rate_limited(EnrichmentSourceError::rate_limited("busy"), JobErrorKind::Retryable)]
#[case::decode(EnrichmentSourceError::decode("invalid response"), JobErrorKind::Fatal)]
#[case::invalid_request(
    EnrichmentSourceError::invalid_request("bad query"),
    JobErrorKind::Fatal
)]
fn enrichment_source_errors_use_the_domain_retry_classification(
    #[case] source_error: EnrichmentSourceError,
    #[case] expected: JobErrorKind,
) {
    assert_eq!(JobErrorKind::from(&source_error), expected);
}

#[rstest]
fn failure_message_is_truncated_without_splitting_utf8() {
    let source = "界".repeat(MAX_JOB_FAILURE_MESSAGE_BYTES);
    let failure = JobFailure::new(JobErrorKind::Fatal, source);

    assert_eq!(failure.kind(), JobErrorKind::Fatal);
    assert_eq!(failure.message(), "界".repeat(341));
    assert_eq!(failure.message().len(), 1023);
    assert!(failure.message().is_char_boundary(failure.message().len()));
    assert!(failure.message().len() <= MAX_JOB_FAILURE_MESSAGE_BYTES);
}

#[rstest]
fn failure_message_at_the_limit_is_preserved() {
    let source = "x".repeat(MAX_JOB_FAILURE_MESSAGE_BYTES);
    let failure = JobFailure::new(JobErrorKind::Retryable, source.clone());

    assert_eq!(failure.message(), source);
}

#[test]
fn policy_handles_a_saturated_exponential_curve_without_exceeding_its_cap() {
    let policy = policy(JobRetryPolicyConfig {
        max_attempts: NonZeroU32::new(10).expect("positive attempt limit"),
        max_delay: Duration::from_secs(8),
        ..valid_policy_config()
    })
    .expect("valid policy should build");
    let jitter = JitterSample::new(0.5).expect("midpoint is valid");

    assert_eq!(
        policy.backoff_delay(NonZeroU32::new(4).expect("positive attempt"), jitter),
        Duration::from_secs(8)
    );
    assert_eq!(
        policy.backoff_delay(NonZeroU32::new(5).expect("positive attempt"), jitter),
        Duration::from_secs(8)
    );
    assert_eq!(
        policy.backoff_delay(
            NonZeroU32::new(u32::MAX).expect("maximum attempt is positive"),
            jitter,
        ),
        Duration::from_secs(8)
    );
    assert!(
        policy.backoff_delay(
            NonZeroU32::new(u32::MAX).expect("maximum attempt is positive"),
            jitter,
        ) <= MAX_BACKOFF_DELAY
    );
}

#[test]
fn exponential_curve_grows_until_the_cap_then_stays_flat() {
    let policy = policy(JobRetryPolicyConfig {
        max_attempts: NonZeroU32::new(5).expect("positive attempt limit"),
        max_delay: Duration::from_secs(4),
        ..valid_policy_config()
    })
    .expect("valid policy should build");
    let jitter = JitterSample::new(0.5).expect("midpoint is valid");
    let delay = |attempt| {
        policy.backoff_delay(
            NonZeroU32::new(attempt).expect("positive test attempt"),
            jitter,
        )
    };

    assert!(delay(1) < delay(2));
    assert!(delay(2) < delay(3));
    assert_eq!(delay(3), Duration::from_secs(4));
    assert_eq!(delay(3), delay(4));
}

#[test]
fn jitter_sample_error_has_a_stable_error_contract() {
    assert_eq!(
        JitterSample::new(f64::NAN),
        Err(JitterSampleError::NotFinite)
    );
}
