//! Guarded PostgreSQL writes for durable Apalis retry scheduling.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use apalis_postgres::PgPool;
use chrono::{DateTime, Utc};
use sqlx::Error as SqlxError;

/// Time source for the retry deadline the scheduler writes.
///
/// The scheduler previously relied on PostgreSQL's `now()`. Taking the instant
/// from an injected clock instead makes the written deadline a value a test can
/// predict, while [`SystemClock`] keeps production behaviour unchanged.
pub trait ClockSource: Send + Sync {
    /// Return the current instant in UTC.
    fn now(&self) -> DateTime<Utc>;
}

/// Production [`ClockSource`] backed by the system clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl ClockSource for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

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

/// Details of one delivery's retry to schedule.
///
/// The fields describe a single delivery, so they travel as one value: passing
/// them positionally would let a call site pair a worker with the wrong task's
/// attempt, which is exactly the write the delivery guard exists to refuse.
pub struct RetryScheduleUpdate<'a> {
    /// Identity of the task to retry.
    pub task_id: &'a str,
    /// Worker that owns the task's lock.
    pub worker_id: &'a str,
    /// Attempt that just failed, as the delivery counts it.
    pub attempt: NonZeroU32,
    /// Delay before the task becomes eligible again.
    pub delay: Duration,
}

/// Details of the fallback write that lowers a row's attempt cap.
///
/// The write exists for one case: a terminal classification whose reason could
/// not be stored. Apalis still acknowledges that delivery as failed, and a
/// failed row stays fetchable while `attempts < max_attempts`, so the cap is
/// lowered to the terminating attempt before that acknowledgement lands.
#[derive(Clone, Copy)]
pub struct TerminalCapClamp<'a> {
    /// Identity of the task that terminated.
    pub task_id: &'a str,
    /// Worker that owns the task's lock.
    pub worker_id: &'a str,
    /// Attempt that terminated the task.
    pub attempt: NonZeroU32,
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
    clock: Arc<dyn ClockSource>,
}

impl PostgresRetryScheduler {
    /// Create a scheduler for the shared Apalis jobs table.
    ///
    /// # Examples
    ///
    /// ```text
    /// let scheduler = PostgresRetryScheduler::new(pool.clone());
    /// let updated = scheduler
    ///     .schedule_retry(RetryScheduleUpdate {
    ///         task_id,
    ///         worker_id,
    ///         attempt: NonZeroU32::new(2).expect("positive attempt"),
    ///         delay,
    ///     })
    ///     .await?;
    /// assert!(updated, "the worker still owns a running task");
    /// ```
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            clock: Arc::new(SystemClock),
        }
    }

    /// Replace the time source used for the retry deadline.
    ///
    /// Production uses [`SystemClock`]. Tests inject a fixed clock so the
    /// written `run_at` is predictable.
    ///
    /// # Examples
    ///
    /// ```text
    /// let scheduler = PostgresRetryScheduler::new(pool.clone())
    ///     .with_clock(Arc::new(FixedClock(now)));
    /// ```
    #[must_use]
    pub fn with_clock(mut self, clock: Arc<dyn ClockSource>) -> Self {
        self.clock = clock;
        self
    }

    /// Move a running, worker-owned task's next delivery time forward.
    ///
    /// Returns `false` when the row is no longer owned by this delivery: it is
    /// not running, another worker holds the lock, or the row has since been
    /// delivered again and its attempt counter moved on. The deadline is taken
    /// from the injected [`ClockSource`] and bound as the new `run_at`.
    ///
    /// # Examples
    ///
    /// ```text
    /// let updated = scheduler
    ///     .schedule_retry(RetryScheduleUpdate {
    ///         task_id,
    ///         worker_id,
    ///         attempt,
    ///         delay: Duration::from_secs(2),
    ///     })
    ///     .await?;
    /// // `false` means the guarded update did not match this worker's row.
    /// ```
    ///
    /// # Errors
    ///
    /// Returns the PostgreSQL error when the guarded update cannot be
    /// executed, or a protocol error when the delay is not representable as a
    /// microsecond offset, the resulting deadline lies outside the supported
    /// timestamp range, or the attempt exceeds Apalis's integer range.
    #[tracing::instrument(
        skip_all,
        fields(
            operation = "schedule_retry",
            task_id = update.task_id,
            worker_id = update.worker_id,
            attempt = update.attempt.get(),
            delay_micros = %update.delay.as_micros(),
            scheduled_at = tracing::field::Empty,
        )
    )]
    pub async fn schedule_retry(&self, update: RetryScheduleUpdate<'_>) -> Result<bool, SqlxError> {
        let RetryScheduleUpdate {
            task_id,
            worker_id,
            attempt,
            delay,
        } = update;
        let delay_micros = i64::try_from(delay.as_micros()).map_err(|_| {
            SqlxError::Protocol("retry delay exceeds PostgreSQL interval range".to_owned())
        })?;
        let bounds = AttemptBounds::for_delivery(attempt)?;
        let scheduled_at = retry_deadline(self.clock.as_ref(), delay_micros)?;
        tracing::Span::current().record("scheduled_at", tracing::field::display(scheduled_at));

        let result = sqlx::query(
            "UPDATE apalis.jobs
             SET run_at = $4
             WHERE id = $1 AND lock_by = $2 AND status = 'Running' AND attempts = $3",
        )
        .bind(task_id)
        .bind(worker_id)
        .bind(bounds.stored)
        .bind(scheduled_at)
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
        let bounds = AttemptBounds::for_delivery(update.attempt)?;
        let result = sqlx::query(
            "UPDATE apalis.jobs
             SET max_attempts = CASE WHEN $3 THEN $4 ELSE max_attempts END,
                 metadata = jsonb_set(
                     COALESCE(metadata, '{}'::jsonb),
                     '{wildside.retry.dead_reason}',
                     to_jsonb($5::text),
                     true
                 )
             WHERE id = $1 AND lock_by = $2 AND status = 'Running' AND attempts = $6",
        )
        .bind(update.task_id)
        .bind(update.worker_id)
        .bind(update.shrink_limit)
        .bind(bounds.acknowledged)
        .bind(update.reason.as_label())
        .bind(bounds.stored)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() == 1)
    }

    /// Lower a row's attempt cap to the attempt that just terminated it.
    ///
    /// This is the fallback for a terminal classification whose reason could
    /// not be stored. It leaves the attempt cap no higher than the terminating
    /// attempt, so the acknowledgement Apalis performs next stores a `Failed`
    /// row that no fetch selects, instead of one the worker would run again.
    /// A cap that is already lower is left alone.
    ///
    /// # Examples
    ///
    /// ```text
    /// let clamped = scheduler.clamp_terminal_cap(TerminalCapClamp {
    ///     task_id,
    ///     worker_id,
    ///     attempt: NonZeroU32::new(3).expect("positive attempt"),
    /// }).await?;
    /// // `false` means another actor has taken the row over.
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
            operation = "clamp_terminal_cap",
            task_id = clamp.task_id,
            worker_id = clamp.worker_id,
            attempt = clamp.attempt.get(),
        )
    )]
    pub(crate) async fn clamp_terminal_cap(
        &self,
        clamp: TerminalCapClamp<'_>,
    ) -> Result<bool, SqlxError> {
        let TerminalCapClamp {
            task_id,
            worker_id,
            attempt,
        } = clamp;
        let bounds = AttemptBounds::for_delivery(attempt)?;
        let result = sqlx::query(
            "UPDATE apalis.jobs
             SET max_attempts = LEAST(COALESCE(max_attempts, $4), $4)
             WHERE id = $1 AND lock_by = $2 AND status = 'Running' AND attempts = $3",
        )
        .bind(task_id)
        .bind(worker_id)
        .bind(bounds.stored)
        .bind(bounds.acknowledged)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() == 1)
    }
}

/// The attempt values a guarded write needs for one delivery.
///
/// Apalis increments the in-memory attempt on the first poll of a delivery, so
/// while a handler runs the row still holds one less than the delivery counts.
/// Guarding a write on [`stored`](Self::stored) binds it to the delivery that
/// produced it: a row handed to a later delivery carries a higher counter and
/// no longer matches. [`acknowledged`](Self::acknowledged) is the value the row
/// takes when Apalis acknowledges this delivery as failed.
#[derive(Clone, Copy, Debug)]
struct AttemptBounds {
    /// Attempt count the row holds during this delivery.
    stored: i32,
    /// Attempt count the row takes once this delivery is acknowledged.
    acknowledged: i32,
}

impl AttemptBounds {
    /// Derive both counts from the attempt the delivery reports.
    ///
    /// A delivery always reports at least one, so the stored count cannot
    /// underflow.
    fn for_delivery(attempt: NonZeroU32) -> Result<Self, SqlxError> {
        let out_of_range =
            || SqlxError::Protocol("retry attempt exceeds Apalis integer range".to_owned());
        Ok(Self {
            stored: i32::try_from(attempt.get() - 1).map_err(|_| out_of_range())?,
            acknowledged: i32::try_from(attempt.get()).map_err(|_| out_of_range())?,
        })
    }
}

/// Compute the retry deadline from the injected clock and a microsecond delay.
fn retry_deadline(clock: &dyn ClockSource, delay_micros: i64) -> Result<DateTime<Utc>, SqlxError> {
    clock
        .now()
        .checked_add_signed(chrono::Duration::microseconds(delay_micros))
        .ok_or_else(|| {
            SqlxError::Protocol("retry delay exceeds PostgreSQL interval range".to_owned())
        })
}

#[cfg(test)]
mod tests;
