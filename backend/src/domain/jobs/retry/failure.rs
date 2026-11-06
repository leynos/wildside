//! Classified job failures and their bounded, payload-free messages.
//!
//! Failure classification belongs to the domain so queue adapters only carry
//! the resulting decision and never inspect job payloads.

use crate::domain::ports::EnrichmentSourceError;

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
