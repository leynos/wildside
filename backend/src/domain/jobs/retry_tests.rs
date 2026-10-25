//! Unit tests for domain retry policy and failure classification.

use std::num::NonZeroU32;
use std::time::Duration;

use googletest::prelude::*;
use pretty_assertions::assert_eq;
use rstest::rstest;

use super::retry::{
    JitterSample, JitterSampleError, JobErrorKind, JobFailure, JobRetryPolicy,
    JobRetryPolicyBuildError, JobRetryPolicyConfig, MAX_BACKOFF_DELAY,
    MAX_JOB_FAILURE_MESSAGE_BYTES, RetryDecision,
};
use crate::domain::ports::EnrichmentSourceError;

mod properties;

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
#[case::below_ceiling(Duration::from_secs(1799), true)]
#[case::exactly_at_ceiling(Duration::from_secs(1800), true)]
#[case::above_ceiling(Duration::from_secs(1801), false)]
fn constructor_checks_the_jittered_delay_ceiling(
    #[case] max_delay: Duration,
    #[case] should_accept: bool,
) {
    let result = policy(JobRetryPolicyConfig {
        multiplier: 1.0,
        max_delay,
        jitter_fraction: 1.0,
        ..valid_policy_config()
    });

    assert_eq!(result.is_ok(), should_accept);
}

#[rstest]
#[case::low_endpoint(0.0)]
#[case::midpoint(0.5)]
#[case::high_endpoint(1.0)]
fn jitter_sample_accepts_the_closed_unit_interval(#[case] sample: f64) {
    assert!(JitterSample::new(sample).is_ok());
}

#[rstest]
#[case::negative(-f64::EPSILON)]
#[case::above_one(1.0 + f64::EPSILON)]
#[case::nan(f64::NAN)]
#[case::positive_infinity(f64::INFINITY)]
#[case::negative_infinity(f64::NEG_INFINITY)]
fn jitter_sample_rejects_values_outside_the_closed_unit_interval(#[case] sample: f64) {
    assert!(JitterSample::new(sample).is_err());
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

#[rstest]
#[case::fatal_below_bound(JobErrorKind::Fatal, 2, true)]
#[case::fatal_at_bound(JobErrorKind::Fatal, 3, true)]
#[case::fatal_above_bound(JobErrorKind::Fatal, 4, true)]
#[case::retryable_below_bound(JobErrorKind::Retryable, 2, false)]
#[case::retryable_at_bound(JobErrorKind::Retryable, 3, true)]
#[case::retryable_above_bound(JobErrorKind::Retryable, 4, true)]
fn decision_table_is_total_and_respects_failure_class_and_attempt_bound(
    #[case] kind: JobErrorKind,
    #[case] attempt: u32,
    #[case] should_dead_letter: bool,
) {
    let policy = policy(valid_policy_config()).expect("valid policy should build");
    let jitter = JitterSample::new(0.5).expect("midpoint is valid");
    let actual = policy.decide(
        NonZeroU32::new(attempt).expect("case attempt is positive"),
        kind,
        jitter,
    );
    let expected = if should_dead_letter {
        RetryDecision::DeadLetter
    } else {
        RetryDecision::Retry {
            delay: Duration::from_secs(2),
        }
    };

    assert_that!(actual, eq(expected));
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
