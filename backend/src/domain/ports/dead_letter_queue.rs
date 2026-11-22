//! Domain port for inspecting and managing jobs after a terminal outcome.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::define_port_error;

define_port_error! {
    /// Failures returned by dead-letter storage operations.
    pub enum DeadLetterError {
        /// The requested job is absent or no longer satisfies the dead predicate.
        NotDead { message: String } => "dead job is not dead: {message}",
        /// The dead job's queue has no configured retry budget.
        UnknownQueue { message: String } => "dead job queue has no retry budget: {message}",
        /// The dead row has invalid attempt or requeue metadata.
        InvalidJobData { message: String } => "dead job has invalid data: {message}",
        /// PostgreSQL could not complete the operation.
        Unavailable { message: String } => "dead-letter storage is unavailable: {message}",
    }
}

/// Opaque identifier for a dead job; its representation belongs to the adapter.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DeadJobId(String);

impl DeadJobId {
    /// Preserve a storage identifier without exposing its format to callers.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Return the identifier in the representation required by the adapter.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Classification of a terminal job outcome.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeadReason {
    /// A retryable failure reached the queue's configured attempt limit.
    Exhausted,
    /// A handler classified the failure as permanent.
    Fatal,
    /// A legacy or malformed row has no recognized terminal reason.
    Unclassified,
}

/// A job parked after a terminal queue outcome.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeadJob {
    /// Storage identifier of the dead job.
    pub id: DeadJobId,
    /// Job type name recorded when the job was enqueued.
    pub job_type: String,
    /// Why the job reached a terminal outcome.
    pub reason: DeadReason,
    /// Attempts consumed before the job went dead.
    pub attempts: u32,
    /// A bounded error string, without the serialized job payload.
    pub last_error: Option<String>,
    /// Number of times the job has been requeued.
    pub requeue_count: u32,
    /// When the job went dead, when the adapter records it.
    pub dead_at: Option<DateTime<Utc>>,
}

/// Why a stored dead-letter row could not be represented as a [`DeadJob`].
///
/// The variants stay coarse so the reason can be used as a bounded log field
/// and matched by callers; the offending value belongs in the row itself, not
/// in this classification.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeadJobSkip {
    /// The stored attempt count is negative, which no attempt count can be.
    NegativeAttempts,
    /// The stored requeue counter is not a valid count.
    InvalidRequeueCount,
}

impl DeadJobSkip {
    /// Return a short, bounded label for logging and test assertions.
    ///
    /// # Examples
    ///
    /// ```
    /// use backend::domain::ports::DeadJobSkip;
    ///
    /// assert_eq!(DeadJobSkip::NegativeAttempts.as_label(), "negative-attempts");
    /// ```
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::NegativeAttempts => "negative-attempts",
            Self::InvalidRequeueCount => "invalid-requeue-count",
        }
    }
}

/// A dead-letter row the listing could not represent, and why.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkippedDeadJob {
    /// Storage identifier of the row that was skipped.
    pub id: DeadJobId,
    /// The field that made the row unrepresentable.
    pub cause: DeadJobSkip,
}

impl SkippedDeadJob {
    /// Pair a row identifier with the reason it was skipped.
    ///
    /// # Examples
    ///
    /// ```
    /// use backend::domain::ports::{DeadJobId, DeadJobSkip, SkippedDeadJob};
    ///
    /// let skipped = SkippedDeadJob::new(DeadJobId::new("row-1"), DeadJobSkip::NegativeAttempts);
    /// assert_eq!(skipped.id.as_str(), "row-1");
    /// ```
    pub fn new(id: DeadJobId, cause: DeadJobSkip) -> Self {
        Self { id, cause }
    }
}

/// The outcome of one dead-job listing.
///
/// A listing reads storage that earlier versions, or concurrent writers, may
/// have left malformed. Rows the domain cannot represent are reported
/// separately instead of being dropped silently, so a caller can tell an empty
/// queue from an unreadable one without consulting logs. Both parts are read
/// through accessors so a caller cannot consume the jobs and lose the skips.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeadLetterListing {
    jobs: Vec<DeadJob>,
    skipped: Vec<SkippedDeadJob>,
}

impl DeadLetterListing {
    /// Combine the represented jobs with the rows that were skipped.
    ///
    /// # Examples
    ///
    /// ```
    /// use backend::domain::ports::DeadLetterListing;
    ///
    /// let listing = DeadLetterListing::new(Vec::new(), Vec::new());
    /// assert!(listing.jobs().is_empty());
    /// assert!(listing.skipped().is_empty());
    /// ```
    pub fn new(jobs: Vec<DeadJob>, skipped: Vec<SkippedDeadJob>) -> Self {
        Self { jobs, skipped }
    }

    /// The dead jobs, newest first.
    pub fn jobs(&self) -> &[DeadJob] {
        &self.jobs
    }

    /// The rows storage held that could not be represented.
    pub fn skipped(&self) -> &[SkippedDeadJob] {
        &self.skipped
    }

    /// Consume the listing, returning its jobs and skipped rows.
    ///
    /// # Examples
    ///
    /// ```
    /// use backend::domain::ports::DeadLetterListing;
    ///
    /// let (jobs, skipped) = DeadLetterListing::new(Vec::new(), Vec::new()).into_parts();
    /// assert!(jobs.is_empty() && skipped.is_empty());
    /// ```
    pub fn into_parts(self) -> (Vec<DeadJob>, Vec<SkippedDeadJob>) {
        (self.jobs, self.skipped)
    }
}

/// Read and manage terminal jobs without exposing PostgreSQL to the domain.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait DeadLetterQueue: Send + Sync {
    /// List newest dead jobs first, returning at most `limit` jobs.
    ///
    /// Rows that storage holds but the domain cannot represent are returned in
    /// [`DeadLetterListing::skipped`] rather than dropped, so a caller can tell
    /// a genuinely short queue from an unreadable one.
    async fn list_dead(&self, limit: u32) -> Result<DeadLetterListing, DeadLetterError>;

    /// Restore the configured retry budget for a currently dead job.
    async fn requeue(&self, id: &DeadJobId) -> Result<(), DeadLetterError>;

    /// Permanently remove a currently dead job.
    async fn discard(&self, id: &DeadJobId) -> Result<(), DeadLetterError>;
}

#[cfg(test)]
mod tests;
