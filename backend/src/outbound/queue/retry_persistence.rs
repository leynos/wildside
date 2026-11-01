//! Port for the guarded writes the retry wrapper performs before acknowledgement.
//!
//! [`RetryingJobService`](super::RetryingJobService) decides whether a failed
//! delivery should be rescheduled or made terminal, then performs exactly one
//! guarded write. Both writes are the only side effects on that path, so the
//! wrapper depends on this small trait rather than the concrete
//! [`PostgresRetryScheduler`]. That keeps the "one write per failure" contract
//! testable with a recording double, and it mirrors how the queue provider is
//! abstracted for the same reason.
//!
//! The port has a single production implementation. Introduce another only if a
//! second durable backing store is actually adopted; do not use it as a generic
//! repository abstraction.

use std::num::NonZeroU32;
use std::time::Duration;

use async_trait::async_trait;
use sqlx::Error as SqlxError;

use super::postgres_retry_scheduler::{
    PostgresRetryScheduler, TerminalRetryReason, TerminalRetryUpdate,
};

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
    async fn record_terminal_failure(
        &self,
        task_id: &str,
        worker_id: &str,
        attempt: NonZeroU32,
        reason: TerminalRetryReason,
        shrink_limit: bool,
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
        task_id: &str,
        worker_id: &str,
        attempt: NonZeroU32,
        reason: TerminalRetryReason,
        shrink_limit: bool,
    ) -> Result<bool, SqlxError> {
        PostgresRetryScheduler::record_terminal_failure(
            self,
            TerminalRetryUpdate {
                task_id,
                worker_id,
                attempt,
                reason,
                shrink_limit,
            },
        )
        .await
    }
}
