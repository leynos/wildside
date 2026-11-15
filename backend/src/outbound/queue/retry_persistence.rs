//! Port for the guarded writes the retry wrapper performs before acknowledgement.
//!
//! [`RetryingJobService`](super::RetryingJobService) decides whether a failed
//! delivery should be rescheduled or made terminal. When it can prove the
//! current worker owns the row it asks for one guarded write, and it writes
//! nothing when the identity is missing or disagrees. Those requests are the
//! only *persistence* writes the wrapper can cause, with one bounded exception:
//! a terminal write that fails is followed by a single cap-lowering write, so
//! the failure Apalis is about to record cannot be fetched and run again. That
//! fallback carries the same delivery guards, so it cannot change a row another
//! actor owns.
//!
//! The wrapper also logs every decision and records the metrics that describe a
//! write outcome. A terminal decision whose delivery identity is missing or
//! disagrees is the exception: it logs an error and records no metric, because
//! no write was attempted. Logging and metrics are deliberately not routed
//! through this port. It depends on this small trait rather than the concrete
//! [`PostgresRetryScheduler`] so the "at most one primary write per failure"
//! contract stays testable with a recording double, mirroring how the queue
//! provider is abstracted for the same reason.
//!
//! The port has a single production implementation. Introduce another only if a
//! second durable backing store is actually adopted; do not use it as a generic
//! repository abstraction.

use async_trait::async_trait;
use sqlx::Error as SqlxError;

use super::postgres_retry_scheduler::{
    PostgresRetryScheduler, RetryScheduleUpdate, TerminalCapClamp, TerminalRetryUpdate,
};

/// Guarded persistence writes required by the retrying Apalis service.
///
/// Every method returns `Ok(false)` when the guarded statement matched no
/// running, worker-owned row for this delivery. That outcome is not an error:
/// another actor has already taken responsibility for the task, and a write
/// must not overwrite its decision.
#[async_trait]
pub trait RetryPersistence: Send + Sync {
    /// Move a running, worker-owned task's next delivery time forward.
    ///
    /// The four fields describing the delivery travel together as a
    /// [`RetryScheduleUpdate`], so a call site cannot pair a worker with
    /// another task's attempt.
    async fn schedule_retry(&self, update: RetryScheduleUpdate<'_>) -> Result<bool, SqlxError>;

    /// Persist a terminal reason and optionally shrink the attempt limit.
    ///
    /// The five columns that describe a terminal failure travel together, so
    /// they are passed as [`TerminalRetryUpdate`] rather than as a long
    /// positional argument list that call sites can silently transpose.
    async fn record_terminal_failure(
        &self,
        update: TerminalRetryUpdate<'_>,
    ) -> Result<bool, SqlxError>;

    /// Lower a row's attempt cap to the attempt that terminated it.
    ///
    /// This is the fallback for [`record_terminal_failure`](Self::record_terminal_failure)
    /// returning an error. The handler is about to acknowledge the delivery as
    /// failed, and a failed row stays fetchable while `attempts < max_attempts`,
    /// so the cap must come down to the terminating attempt or the job the
    /// policy retired would run again.
    async fn clamp_terminal_cap(&self, clamp: TerminalCapClamp<'_>) -> Result<bool, SqlxError>;
}

#[async_trait]
impl RetryPersistence for PostgresRetryScheduler {
    async fn schedule_retry(&self, update: RetryScheduleUpdate<'_>) -> Result<bool, SqlxError> {
        PostgresRetryScheduler::schedule_retry(self, update).await
    }

    async fn record_terminal_failure(
        &self,
        update: TerminalRetryUpdate<'_>,
    ) -> Result<bool, SqlxError> {
        PostgresRetryScheduler::record_terminal_failure(self, update).await
    }

    async fn clamp_terminal_cap(&self, clamp: TerminalCapClamp<'_>) -> Result<bool, SqlxError> {
        PostgresRetryScheduler::clamp_terminal_cap(self, clamp).await
    }
}
