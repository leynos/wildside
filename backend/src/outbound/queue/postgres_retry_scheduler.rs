//! Guarded PostgreSQL writes for durable Apalis retry scheduling.

use std::num::NonZeroU32;
use std::time::Duration;

use apalis_postgres::PgPool;
use sqlx::Error as SqlxError;

/// Terminal classification stored in D15's namespaced metadata field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalRetryReason {
    /// The handler classified the error as permanent.
    Fatal,
    /// A retryable error reached a configured attempt limit.
    Exhausted,
}

/// Details persisted when a task reaches a terminal retry outcome.
///
/// The port accepts this value rather than five positional arguments so a
/// call site cannot transpose the columns that describe one row.
pub struct TerminalRetryUpdate<'a> {
    /// Identity of the task that terminated.
    pub task_id: &'a str,
    /// Worker that owns the task's lock.
    pub worker_id: &'a str,
    /// Attempt that terminated the task.
    pub attempt: NonZeroU32,
    /// Why the task will not be retried.
    pub reason: TerminalRetryReason,
    /// Whether the attempt limit shrinks to the terminating attempt.
    pub shrink_limit: bool,
}

impl TerminalRetryReason {
    /// Stable value stored under `wildside.retry.dead_reason`.
    ///
    /// # Examples
    ///
    /// ```
    /// use backend::outbound::queue::TerminalRetryReason;
    ///
    /// assert_eq!(TerminalRetryReason::Exhausted.as_label(), "exhausted");
    /// ```
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Fatal => "fatal",
            Self::Exhausted => "exhausted",
        }
    }
}

/// Performs the guarded PostgreSQL writes needed by the Apalis retry wrapper.
#[derive(Clone)]
pub struct PostgresRetryScheduler {
    pool: PgPool,
}

impl PostgresRetryScheduler {
    /// Create a scheduler for the shared Apalis jobs table.
    ///
    /// # Examples
    ///
    /// ```text
    /// let scheduler = PostgresRetryScheduler::new(pool.clone());
    /// let updated = scheduler
    ///     .schedule_retry(task_id, worker_id, delay)
    ///     .await?;
    /// assert!(updated, "the worker still owns a running task");
    /// ```
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Move a running, worker-owned task's next delivery time forward.
    ///
    /// Returns `false` when the row is no longer owned by this worker or is not
    /// running. The update uses microseconds because `Duration` is not a
    /// PostgreSQL parameter type.
    ///
    /// # Examples
    ///
    /// ```text
    /// let updated = scheduler
    ///     .schedule_retry(task_id, worker_id, Duration::from_secs(2))
    ///     .await?;
    /// // `false` means the guarded update did not match this worker's row.
    /// ```
    ///
    /// # Errors
    ///
    /// Returns the PostgreSQL error when the guarded update cannot be
    /// executed, or a protocol error when the delay exceeds the interval
    /// range accepted by PostgreSQL.
    #[tracing::instrument(
        skip_all,
        fields(
            operation = "schedule_retry",
            task_id,
            worker_id,
            delay_micros = %delay.as_micros(),
        )
    )]
    pub async fn schedule_retry(
        &self,
        task_id: &str,
        worker_id: &str,
        delay: Duration,
    ) -> Result<bool, SqlxError> {
        let delay_micros = i64::try_from(delay.as_micros()).map_err(|_| {
            SqlxError::Protocol("retry delay exceeds PostgreSQL interval range".to_owned())
        })?;
        let result = sqlx::query(
            "UPDATE apalis.jobs
             SET run_at = now() + ($3::bigint * interval '1 microsecond')
             WHERE id = $1 AND lock_by = $2 AND status = 'Running'",
        )
        .bind(task_id)
        .bind(worker_id)
        .bind(delay_micros)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() == 1)
    }

    /// Persist a terminal reason and optionally shrink the stored attempt cap.
    ///
    /// The guarded statement runs before Apalis acknowledgement. Shrinking the
    /// cap only when requested makes the later acknowledgement leave a
    /// `Failed` row unfetchable when the policy limit is lower than the row's
    /// existing cap. The metadata update preserves unrelated keys.
    ///
    /// # Examples
    ///
    /// ```text
    /// let recorded = scheduler.record_terminal_failure(TerminalRetryUpdate {
    ///     task_id,
    ///     worker_id,
    ///     attempt: NonZeroU32::new(3).expect("positive attempt"),
    ///     reason: TerminalRetryReason::Exhausted,
    ///     shrink_limit: true,
    /// }).await?;
    /// assert!(recorded, "the worker still owns a running task");
    /// ```
    ///
    /// # Errors
    ///
    /// Returns the PostgreSQL error when the guarded update cannot be
    /// executed, or a protocol error when the attempt exceeds Apalis's integer
    /// range.
    #[tracing::instrument(
        skip_all,
        fields(
            operation = "record_terminal_failure",
            task_id = update.task_id,
            worker_id = update.worker_id,
            attempt = update.attempt.get(),
            reason = update.reason.as_label(),
            shrink_limit = update.shrink_limit,
        )
    )]
    pub(crate) async fn record_terminal_failure(
        &self,
        update: TerminalRetryUpdate<'_>,
    ) -> Result<bool, SqlxError> {
        let attempt = i32::try_from(update.attempt.get()).map_err(|_| {
            SqlxError::Protocol("retry attempt exceeds Apalis integer range".to_owned())
        })?;
        let result = sqlx::query(
            "UPDATE apalis.jobs
             SET max_attempts = CASE WHEN $3 THEN $4 ELSE max_attempts END,
                 metadata = jsonb_set(
                     COALESCE(metadata, '{}'::jsonb),
                     '{wildside.retry.dead_reason}',
                     to_jsonb($5::text),
                     true
                 )
             WHERE id = $1 AND lock_by = $2 AND status = 'Running'",
        )
        .bind(update.task_id)
        .bind(update.worker_id)
        .bind(update.shrink_limit)
        .bind(attempt)
        .bind(update.reason.as_label())
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() == 1)
    }
}
