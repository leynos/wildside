//! Property tests for retry-policy bounds and monotonicity.

use std::cell::Cell;
use std::num::NonZeroU32;
use std::time::Duration;

use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};

use super::super::retry::{
    JitterSample, JobErrorKind, JobRetryPolicy, JobRetryPolicyConfig, MAX_BACKOFF_DELAY,
    RetryDecision,
};

fn policy_inputs() -> impl Strategy<
    Value = (
        JobRetryPolicy,
        NonZeroU32,
        JitterSample,
        f64,
        Duration,
        f64,
        Duration,
    ),
> {
    (
        1u32..=100,
        1u64..=60_000,
        1_000u32..=10_000,
        1u64..=1_800,
        0u32..=10_000,
        0u32..=10_000,
        1u32..=100,
    )
        .prop_map(
            |(
                max_attempts,
                base_delay_millis,
                multiplier_milli,
                max_delay_secs,
                jitter_fraction_basis_points,
                jitter_sample_basis_points,
                attempt,
            )| {
                let jitter_fraction = f64::from(jitter_fraction_basis_points) / 10_000.0;
                let policy = JobRetryPolicy::new(JobRetryPolicyConfig {
                    max_attempts: NonZeroU32::new(max_attempts)
                        .expect("generated limit is positive"),
                    base_delay: Duration::from_millis(base_delay_millis),
                    multiplier: f64::from(multiplier_milli) / 1_000.0,
                    max_delay: Duration::from_secs(max_delay_secs),
                    jitter_fraction,
                })
                .expect("generated policy respects the one-hour jittered cap");
                let attempt = NonZeroU32::new(attempt.min(max_attempts))
                    .expect("generated attempt is positive");
                let jitter_sample = f64::from(jitter_sample_basis_points) / 10_000.0;
                let jitter =
                    JitterSample::new(jitter_sample).expect("generated jitter sample is valid");
                (
                    policy,
                    attempt,
                    jitter,
                    jitter_fraction,
                    Duration::from_millis(base_delay_millis),
                    f64::from(multiplier_milli) / 1_000.0,
                    Duration::from_secs(max_delay_secs),
                )
            },
        )
}

proptest! {
    #[test]
    fn backoff_delay_stays_inside_the_jittered_exponential_bounds(
        (policy, attempt, jitter, jitter_fraction, base_delay, multiplier, max_delay) in policy_inputs()
    ) {
        let uncapped = base_delay.as_secs_f64() * multiplier.powf(f64::from(attempt.get() - 1));
        let nominal = uncapped.min(max_delay.as_secs_f64());
        let lower_bound = nominal * (1.0 - jitter_fraction);
        let upper_bound = nominal * (1.0 + jitter_fraction);
        let actual = policy.backoff_delay(attempt, jitter).as_secs_f64();

        prop_assert!(actual + 2e-9 >= lower_bound);
        prop_assert!(actual <= upper_bound + 2e-9);

        let largest_attempt = NonZeroU32::new(u32::MAX)
            .expect("maximum attempt is positive");
        let midpoint = JitterSample::new(0.5).expect("midpoint is valid");
        prop_assert!(policy.backoff_delay(largest_attempt, midpoint) <= max_delay);
    }

    #[test]
    fn every_accepted_policy_stays_below_the_hard_delay_ceiling(
        (policy, attempt, jitter, ..) in policy_inputs()
    ) {
        prop_assert!(policy.backoff_delay(attempt, jitter) <= MAX_BACKOFF_DELAY);
    }

    #[test]
    fn midpoint_backoff_is_monotone_for_generated_policies(
        (policy, attempt, ..) in policy_inputs()
    ) {
        let midpoint = JitterSample::new(0.5).expect("midpoint is valid");
        let next_attempt = NonZeroU32::new(attempt.get() + 1)
            .expect("generated attempt is below u32::MAX");
        let current = policy.backoff_delay(attempt, midpoint);
        let next = policy.backoff_delay(next_attempt, midpoint);

        prop_assert!(next >= current);
    }

    #[test]
    fn decision_is_dead_exactly_for_fatal_or_exhausted_attempts(
        max_attempts in 1u32..=100,
        attempt in 1u32..=u32::MAX,
        is_fatal in any::<bool>(),
        sample_basis_points in 0u32..=10_000,
    ) {
        let policy = JobRetryPolicy::new(JobRetryPolicyConfig {
            max_attempts: NonZeroU32::new(max_attempts)
                .expect("generated attempt limit is positive"),
            base_delay: Duration::from_millis(10),
            multiplier: 2.0,
            max_delay: Duration::from_secs(60),
            jitter_fraction: 0.25,
        })
        .expect("test policy remains well below the hard delay ceiling");
        let attempt = NonZeroU32::new(attempt).expect("generated attempt is positive");
        let kind = if is_fatal {
            JobErrorKind::Fatal
        } else {
            JobErrorKind::Retryable
        };
        let jitter = JitterSample::new(f64::from(sample_basis_points) / 10_000.0)
            .expect("generated jitter is in the closed unit interval");
        let decision = policy.decide(attempt, kind, jitter);
        let should_dead_letter = is_fatal || attempt.get() >= max_attempts;

        match decision {
            RetryDecision::DeadLetter => {
                prop_assert!(should_dead_letter);
            }
            RetryDecision::Retry { delay } => {
                prop_assert!(!should_dead_letter);
                prop_assert!(delay <= MAX_BACKOFF_DELAY);
            }
        }
    }

    #[test]
    fn accepts_generated_configurations_inside_the_jittered_ceiling(
        jitter_percent in 0u32..=100,
        delay_offset in 0u64..=3600,
    ) {
        let jitter_fraction = f64::from(jitter_percent) / 100.0;
        let largest_valid_secs = (3599.0 / (1.0 + jitter_fraction)).floor() as u64;
        let max_delay_secs = 1 + delay_offset % largest_valid_secs;
        let policy = JobRetryPolicy::new(JobRetryPolicyConfig {
            max_attempts: NonZeroU32::new(3).expect("positive attempt limit"),
            base_delay: Duration::from_secs(1),
            multiplier: 1.0,
            max_delay: Duration::from_secs(max_delay_secs),
            jitter_fraction,
        });

        prop_assert!(policy.is_ok());
    }

    #[test]
    fn rejects_generated_configurations_above_the_jittered_ceiling(
        jitter_percent in 1u32..=100,
    ) {
        let jitter_fraction = f64::from(jitter_percent) / 100.0;
        let first_invalid_secs = (3600.0 / (1.0 + jitter_fraction)).floor() as u64 + 2;
        let policy = JobRetryPolicy::new(JobRetryPolicyConfig {
            max_attempts: NonZeroU32::new(3).expect("positive attempt limit"),
            base_delay: Duration::from_secs(1),
            multiplier: 1.0,
            max_delay: Duration::from_secs(first_invalid_secs),
            jitter_fraction,
        });

        prop_assert!(policy.is_err());
    }
}

#[test]
fn midpoint_monotonicity_generator_records_capped_and_uncapped_cases() {
    let cases = prop_oneof![
        (1u32..=2).prop_map(|attempt| {
            (
                Duration::from_secs(1),
                2.0_f64,
                Duration::from_secs(16),
                attempt,
                false,
            )
        }),
        (2u32..=3).prop_map(|attempt| {
            (
                Duration::from_secs(1),
                2.0_f64,
                Duration::from_secs(4),
                attempt,
                true,
            )
        }),
    ];
    let counts = Cell::new([0_u32; 2]);
    let mut runner = TestRunner::new(Config {
        cases: 128,
        ..Config::default()
    });

    runner
        .run(
            &cases,
            |(base_delay, multiplier, max_delay, attempt, should_be_capped)| {
                let attempt = NonZeroU32::new(attempt).expect("generated attempt is positive");
                let next_attempt = NonZeroU32::new(attempt.get() + 1)
                    .expect("generated attempt is below u32::MAX");
                let next_uncapped_delay =
                    base_delay.as_secs_f64() * multiplier.powf(f64::from(attempt.get()));
                let is_capped = next_uncapped_delay >= max_delay.as_secs_f64();
                prop_assert_eq!(is_capped, should_be_capped);

                let policy = JobRetryPolicy::new(JobRetryPolicyConfig {
                    max_attempts: NonZeroU32::new(5).expect("positive attempt limit"),
                    base_delay,
                    multiplier,
                    max_delay,
                    jitter_fraction: 0.0,
                })
                .expect("generated policy is valid");
                let midpoint = JitterSample::new(0.5).expect("midpoint is valid");
                let current = policy.backoff_delay(attempt, midpoint);
                let next = policy.backoff_delay(next_attempt, midpoint);
                prop_assert!(next >= current);

                let mut generated_counts = counts.get();
                generated_counts[usize::from(should_be_capped)] += 1;
                counts.set(generated_counts);
                Ok(())
            },
        )
        .expect("generated midpoint curves remain monotone");

    let [uncapped, capped] = counts.get();
    assert!(uncapped > 0, "the generator must produce uncapped curves");
    assert!(capped > 0, "the generator must produce capped curves");
    eprintln!("generated curve classes: uncapped={uncapped}, capped={capped}");
}
