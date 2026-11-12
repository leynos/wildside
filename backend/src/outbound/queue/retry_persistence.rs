//! Port for the guarded writes the retry wrapper performs before acknowledgement.
//!
//! [`RetryingJobService`](super::RetryingJobService) decides whether a failed
//! delivery should be rescheduled or made terminal. When it can prove the
//! current worker owns the row it attempts exactly one guarded write, and it
//! writes nothing when the identity is missing or disagrees, or when the
//! database guard refuses the update. Those attempts are the only *persistence*
//! writes the wrapper can cause; it also records a metric and a log line for
//! each decision, and those are deliberately not routed through this port. It
//! depends on this small trait rather than the concrete
//! [`PostgresRetryScheduler`] so the "at most one write per failure" contract
//! stays testable with a recording double, mirroring how the queue provider is
//! abstracted for the same reason.
//!
//! The port has a single production implementation. Introduce another only if a
//! second durable backing store is actually adopted; do not use it as a generic
//! repository abstraction.

use std::time::Duration;

use async_trait::async_trait;
use sqlx::Error as SqlxError;

use super::postgres_retry_scheduler::{PostgresRetryScheduler, TerminalRetryUpdate};

/// Guarded persistence writes required by the retrying Apalis service.
///
/// Both methods return `Ok(false)` when the guarded statement matched no
/// running, worker-owned row. That outcome is not an error: another actor has
/// already taken responsibility for the task.
#[async_trait]
pub trait RetryPersistence: Send + Sync {
    /// Move a running, worker-owned task's next delivery time forward.
    async fn schedule_retry(
        &self,
        task_id: &str,
        worker_id: &str,
        delay: Duration,
    ) -> Result<bool, SqlxError>;

    /// Persist a terminal reason and optionally shrink the attempt limit.
    ///
    /// The five columns that describe a terminal failure travel together, so
    /// they are passed as [`TerminalRetryUpdate`] rather than as a long
    /// positional argument list that call sites can silently transpose.
    async fn record_terminal_failure(
        &self,
        update: TerminalRetryUpdate<'_>,
    ) -> Result<bool, SqlxError>;
}

#[async_trait]
impl RetryPersistence for PostgresRetryScheduler {
    async fn schedule_retry(
        &self,
        task_id: &str,
        worker_id: &str,
        delay: Duration,
    ) -> Result<bool, SqlxError> {
        PostgresRetryScheduler::schedule_retry(self, task_id, worker_id, delay).await
    }

    async fn record_terminal_failure(
        &self,
        update: TerminalRetryUpdate<'_>,
    ) -> Result<bool, SqlxError> {
        PostgresRetryScheduler::record_terminal_failure(self, update).await
    }
}
