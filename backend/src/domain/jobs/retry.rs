//! Pure retry policy and classified failure types for background jobs.
//!
//! The domain decides whether a failure is retryable and how long a retry
//! should wait. Queue adapters supply jitter samples and perform scheduling.

use std::num::NonZeroU32;
use std::time::Duration;

use thiserror::Error;

use crate::domain::ports::EnrichmentSourceError;

/// Maximum delay for one retry, regardless of policy configuration.
pub const MAX_BACKOFF_DELAY: Duration = Duration::from_secs(60 * 60);

/// Maximum UTF-8 byte length retained in a job failure message.
pub const MAX_JOB_FAILURE_MESSAGE_BYTES: usize = 1024;

/// Classification used by the retry policy to decide whether another attempt
/// can help.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobErrorKind {
    /// A transient failure that may succeed if retried later.
    Retryable,
    /// A permanent failure that will not succeed if retried.
    Fatal,
}

impl From<&EnrichmentSourceError> for JobErrorKind {
    fn from(error: &EnrichmentSourceError) -> Self {
        if error.is_retryable() {
            Self::Retryable
        } else {
            Self::Fatal
        }
    }
}

/// A classified failure without job payload data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobFailure {
    kind: JobErrorKind,
    message: String,
}

impl JobFailure {
    /// Create a failure and truncate its message at a UTF-8 boundary.
    ///
    /// The message must be a payload-free diagnostic suitable for the queue's
    /// plain last-result field. This constructor bounds it to
    /// [`MAX_JOB_FAILURE_MESSAGE_BYTES`].
    ///
    /// # Examples
    ///
    /// ```
    /// use backend::domain::jobs::retry::{
    ///     JobErrorKind, JobFailure, MAX_JOB_FAILURE_MESSAGE_BYTES,
    /// };
    ///
    /// let failure = JobFailure::new(JobErrorKind::Fatal, "invalid response");
    /// assert_eq!(failure.message(), "invalid response");
    /// assert!(failure.message().len() <= MAX_JOB_FAILURE_MESSAGE_BYTES);
    /// ```
    pub fn new(kind: JobErrorKind, message: impl Into<String>) -> Self {
        let mut message = message.into();
        if message.len() > MAX_JOB_FAILURE_MESSAGE_BYTES {
            let mut end = MAX_JOB_FAILURE_MESSAGE_BYTES;
            while !message.is_char_boundary(end) {
                end -= 1;
            }
            message.truncate(end);
        }

        Self { kind, message }
    }

    /// Return the failure classification.
    ///
    /// # Examples
    ///
    /// ```
    /// use backend::domain::jobs::retry::{JobErrorKind, JobFailure};
    ///
    /// let failure = JobFailure::new(JobErrorKind::Fatal, "invalid response");
    /// assert_eq!(failure.kind(), JobErrorKind::Fatal);
    /// ```
    pub fn kind(&self) -> JobErrorKind {
        self.kind
    }

    /// Return the bounded, payload-free diagnostic message.
    ///
    /// # Examples
    ///
    /// ```
    /// use backend::domain::jobs::retry::{JobErrorKind, JobFailure};
    ///
    /// let failure = JobFailure::new(JobErrorKind::Retryable, "source unavailable");
    /// assert_eq!(failure.message(), "source unavailable");
    /// ```
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// A validated jitter sample from the closed unit interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JitterSample(f64);

impl JitterSample {
    /// Construct a sample in the inclusive range `[0, 1]`.
    ///
    /// # Examples
    ///
    /// ```
    /// use backend::domain::jobs::retry::JitterSample;
    ///
    /// let midpoint = JitterSample::new(0.5).expect("the midpoint is valid");
    /// ```
    pub fn new(sample: f64) -> Result<Self, JitterSampleError> {
        if !sample.is_finite() {
            return Err(JitterSampleError::NotFinite);
        }
        if !(0.0..=1.0).contains(&sample) {
            return Err(JitterSampleError::OutOfRange);
        }

        Ok(Self(sample))
    }
}

/// Why a jitter sample could not be constructed.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum JitterSampleError {
    /// The input was NaN or infinite.
    #[error("jitter sample must be finite")]
    NotFinite,
    /// The finite input was outside the inclusive unit interval.
    #[error("jitter sample must be in the range 0..=1")]
    OutOfRange,
}

/// Configuration error returned when a retry policy cannot be bounded safely.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum JobRetryPolicyBuildError {
    /// The base delay must be positive.
    #[error("retry base delay must be greater than zero")]
    ZeroBaseDelay,
    /// The multiplier must be finite and at least one.
    #[error("retry multiplier must be finite and at least 1")]
    InvalidMultiplier,
    /// The maximum delay must be positive.
    #[error("retry maximum delay must be greater than zero")]
    ZeroMaxDelay,
    /// The configured maximum delay exceeds the hard per-delay ceiling.
    #[error("retry maximum delay exceeds MAX_BACKOFF_DELAY")]
    MaxDelayExceedsMaximum,
    /// The jitter fraction must be finite and in the inclusive range `[0, 1]`.
    #[error("retry jitter fraction must be finite and in the range 0..=1")]
    InvalidJitterFraction,
    /// The configured maximum delay plus its largest jitter exceeds the ceiling.
    #[error("retry maximum delay including jitter exceeds MAX_BACKOFF_DELAY")]
    JitteredDelayExceedsMaximum,
}

/// Deterministic decision produced from a retry policy and failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetryDecision {
    /// Retry after the given delay.
    Retry { delay: Duration },
    /// Stop retrying and dead-letter the job.
    DeadLetter,
}

/// Exponential retry policy with a validated maximum delay and jitter range.
#[derive(Clone, Debug, PartialEq)]
pub struct JobRetryPolicy {
    max_attempts: NonZeroU32,
    base_delay: Duration,
    multiplier: f64,
    max_delay: Duration,
    jitter_fraction: f64,
}

/// Configuration values validated when building a [`JobRetryPolicy`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JobRetryPolicyConfig {
    /// Maximum number of attempts, including the initial attempt.
    pub max_attempts: NonZeroU32,
    /// Uncapped starting delay; the first retry is clamped to `max_delay`.
    pub base_delay: Duration,
    /// Exponential growth factor, at least `1.0`.
    pub multiplier: f64,
    /// Nominal delay ceiling before jitter is applied.
    pub max_delay: Duration,
    /// Symmetric jitter range as a fraction in `0.0..=1.0`.
    pub jitter_fraction: f64,
}

impl JobRetryPolicy {
    /// Build a retry policy whose largest jittered delay does not exceed the
    /// one-hour hard ceiling.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::num::NonZeroU32;
    /// use std::time::Duration;
    ///
    /// use backend::domain::jobs::retry::{JobRetryPolicy, JobRetryPolicyConfig};
    ///
    /// let policy = JobRetryPolicy::new(JobRetryPolicyConfig {
    ///     max_attempts: NonZeroU32::new(5).expect("positive attempt limit"),
    ///     base_delay: Duration::from_secs(1),
    ///     multiplier: 2.0,
    ///     max_delay: Duration::from_secs(60),
    ///     jitter_fraction: 0.2,
    /// })
    /// .expect("bounded retry policy");
    /// assert_eq!(policy.max_attempts().get(), 5);
    /// ```
    pub fn new(config: JobRetryPolicyConfig) -> Result<Self, JobRetryPolicyBuildError> {
        let JobRetryPolicyConfig {
            max_attempts,
            base_delay,
            multiplier,
            max_delay,
            jitter_fraction,
        } = config;

        if base_delay.is_zero() {
            return Err(JobRetryPolicyBuildError::ZeroBaseDelay);
        }
        if !multiplier.is_finite() || multiplier < 1.0 {
            return Err(JobRetryPolicyBuildError::InvalidMultiplier);
        }
        if max_delay.is_zero() {
            return Err(JobRetryPolicyBuildError::ZeroMaxDelay);
        }
        if max_delay > MAX_BACKOFF_DELAY {
            return Err(JobRetryPolicyBuildError::MaxDelayExceedsMaximum);
        }
        if !jitter_fraction.is_finite() || !(0.0..=1.0).contains(&jitter_fraction) {
            return Err(JobRetryPolicyBuildError::InvalidJitterFraction);
        }

        let largest_jittered_delay = max_delay.as_secs_f64() * (1.0 + jitter_fraction);
        if largest_jittered_delay > MAX_BACKOFF_DELAY.as_secs_f64() {
            return Err(JobRetryPolicyBuildError::JitteredDelayExceedsMaximum);
        }

        Ok(Self {
            max_attempts,
            base_delay,
            multiplier,
            max_delay,
            jitter_fraction,
        })
    }

    /// Decide whether to retry a classified failure at the supplied attempt.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::num::NonZeroU32;
    /// use std::time::Duration;
    ///
    /// use backend::domain::jobs::retry::{
    ///     JobErrorKind, JobRetryPolicy, JobRetryPolicyConfig, JitterSample, RetryDecision,
    /// };
    ///
    /// let policy = JobRetryPolicy::new(JobRetryPolicyConfig {
    ///     max_attempts: NonZeroU32::new(3).expect("positive attempt limit"),
    ///     base_delay: Duration::from_secs(1),
    ///     multiplier: 2.0,
    ///     max_delay: Duration::from_secs(10),
    ///     jitter_fraction: 0.0,
    /// })
    /// .expect("valid policy configuration");
    /// let decision = policy.decide(
    ///     NonZeroU32::new(1).expect("positive attempt"),
    ///     JobErrorKind::Retryable,
    ///     JitterSample::new(0.5).expect("midpoint is valid"),
    /// );
    /// assert_eq!(decision, RetryDecision::Retry { delay: Duration::from_secs(1) });
    /// ```
    pub fn decide(
        &self,
        attempt: NonZeroU32,
        kind: JobErrorKind,
        jitter: JitterSample,
    ) -> RetryDecision {
        if kind == JobErrorKind::Fatal || attempt >= self.max_attempts {
            RetryDecision::DeadLetter
        } else {
            RetryDecision::Retry {
                delay: self.backoff_delay(attempt, jitter),
            }
        }
    }

    /// Calculate the exponential delay after applying the validated jitter.
    ///
    /// The exponent starts at zero for attempt one. Large finite or overflowing
    /// curves saturate at the configured maximum before jitter is applied.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::num::NonZeroU32;
    /// use std::time::Duration;
    ///
    /// use backend::domain::jobs::retry::{
    ///     JobRetryPolicy, JobRetryPolicyConfig, JitterSample,
    /// };
    ///
    /// let policy = JobRetryPolicy::new(JobRetryPolicyConfig {
    ///     max_attempts: NonZeroU32::new(4).expect("positive attempt limit"),
    ///     base_delay: Duration::from_secs(1),
    ///     multiplier: 2.0,
    ///     max_delay: Duration::from_secs(8),
    ///     jitter_fraction: 0.0,
    /// })
    /// .expect("valid policy configuration");
    /// assert_eq!(
    ///     policy.backoff_delay(
    ///         NonZeroU32::new(4).expect("positive attempt"),
    ///         JitterSample::new(0.5).expect("midpoint is valid"),
    ///     ),
    ///     Duration::from_secs(8),
    /// );
    /// ```
    pub fn backoff_delay(&self, attempt: NonZeroU32, jitter: JitterSample) -> Duration {
        let exponent = f64::from(attempt.get() - 1);
        let uncapped_delay = self.base_delay.as_secs_f64() * self.multiplier.powf(exponent);
        let nominal_delay = if uncapped_delay.is_finite() {
            Duration::from_secs_f64(uncapped_delay.min(self.max_delay.as_secs_f64()))
        } else {
            self.max_delay
        };

        let jitter_offset = 2.0 * jitter.0 - 1.0;
        let jitter_factor = 1.0 + self.jitter_fraction * jitter_offset;
        let jittered_seconds = nominal_delay.as_secs_f64() * jitter_factor;

        Duration::from_secs_f64(jittered_seconds.min(MAX_BACKOFF_DELAY.as_secs_f64()))
    }

    /// Return the maximum number of attempts, including the initial attempt.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::num::NonZeroU32;
    /// use std::time::Duration;
    ///
    /// use backend::domain::jobs::retry::{JobRetryPolicy, JobRetryPolicyConfig};
    ///
    /// let policy = JobRetryPolicy::new(JobRetryPolicyConfig {
    ///     max_attempts: NonZeroU32::new(3).expect("positive attempt limit"),
    ///     base_delay: Duration::from_millis(100),
    ///     multiplier: 2.0,
    ///     max_delay: Duration::from_secs(10),
    ///     jitter_fraction: 0.0,
    /// })
    /// .expect("valid policy configuration");
    /// assert_eq!(policy.max_attempts().get(), 3);
    /// ```
    pub fn max_attempts(&self) -> NonZeroU32 {
        self.max_attempts
    }
}
