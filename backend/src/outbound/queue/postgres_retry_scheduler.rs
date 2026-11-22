//! Guarded PostgreSQL writes for durable Apalis retry scheduling.

mod updates;
pub use updates::{RetryScheduleUpdate, TerminalCapClamp, TerminalRetryUpdate};

use std::num::NonZeroU32;
use std::sync::Arc;

use apalis_postgres::PgPool;
use chrono::{DateTime, Utc};
use sqlx::Error as SqlxError;

use crate::domain::ports::DeadReason;

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

/// The text stored under `wildside.retry.dead_reason`, if storage has one.
///
/// `Unclassified` describes a row read back without a recognized reason. It is
/// a read-side classification, so writing it would invent a terminal outcome;
/// callers must refuse it rather than store a sentinel. Keeping the mapping
/// here, beside the statement that writes it, leaves the domain free of
/// storage vocabulary.
const fn stored_dead_reason_label(reason: DeadReason) -> Option<&'static str> {
    match reason {
        DeadReason::Fatal => Some("fatal"),
        DeadReason::Exhausted => Some("exhausted"),
        DeadReason::Unclassified => None,
    }
}

/// Bounded log label for any terminal reason.
///
/// Logging also has to name the refused `Unclassified` case, but that text must
/// not be mistaken for a value storage could hold, so it names the
/// classification defect directly rather than reusing a stored label.
pub(super) const fn dead_reason_log_label(reason: DeadReason) -> &'static str {
    match stored_dead_reason_label(reason) {
        Some(label) => label,
        None => "unclassified",
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
    ///         claim_token,
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
    /// not running, another worker holds the lock, the row has since been
    /// delivered again and its attempt counter moved on, or a later claim
    /// replaced the token this delivery claimed under. The deadline is taken
    /// from the injected [`ClockSource`] and bound as the new `run_at`.
    ///
    /// # Examples
    ///
    /// ```text
    /// let updated = scheduler
    ///     .schedule_retry(RetryScheduleUpdate {
    ///         task_id,
    ///         worker_id,
    ///         claim_token,
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
            claim_token,
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
             WHERE id = $1 AND lock_by = $2 AND status = 'Running' AND attempts = $3
               AND metadata->>'wildside.retry.claim_token' = $5",
        )
        .bind(task_id)
        .bind(worker_id)
        .bind(bounds.stored)
        .bind(scheduled_at)
        .bind(claim_token)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() == 1)
    }

    /// Persist a terminal reason and optionally shrink the stored attempt cap.
    ///
    /// The guarded statement runs before Apalis acknowledgement. Shrinking the
    /// cap only when requested makes the later acknowledgement leave a
    /// `Failed` row unfetchable when the policy limit is lower than the row's
    /// existing cap. The metadata update preserves unrelated keys, including
    /// the claim token the predicate requires.
    ///
    /// # Examples
    ///
    /// ```text
    /// let recorded = scheduler.record_terminal_failure(TerminalRetryUpdate {
    ///     task_id,
    ///     worker_id,
    ///     claim_token,
    ///     attempt: NonZeroU32::new(3).expect("positive attempt"),
    ///     reason: DeadReason::Exhausted,
    ///     shrink_limit: true,
    /// }).await?;
    /// assert!(recorded, "the worker still owns a running task");
    /// ```
    ///
    /// # Errors
    ///
    /// Returns the PostgreSQL error when the guarded update cannot be
    /// executed, or a protocol error when the reason has no stored label or
    /// the attempt exceeds Apalis's integer range.
    #[tracing::instrument(
        skip_all,
        fields(
            operation = "record_terminal_failure",
            task_id = update.task_id,
            worker_id = update.worker_id,
            attempt = update.attempt.get(),
            reason = dead_reason_log_label(update.reason),
            shrink_limit = update.shrink_limit,
        )
    )]
    pub(crate) async fn record_terminal_failure(
        &self,
        update: TerminalRetryUpdate<'_>,
    ) -> Result<bool, SqlxError> {
        let stored_reason = stored_dead_reason_label(update.reason).ok_or_else(|| {
            SqlxError::Protocol("unclassified dead reason cannot be stored".to_owned())
        })?;
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
             WHERE id = $1 AND lock_by = $2 AND status = 'Running' AND attempts = $6
               AND metadata->>'wildside.retry.claim_token' = $7",
        )
        .bind(update.task_id)
        .bind(update.worker_id)
        .bind(update.shrink_limit)
        .bind(bounds.acknowledged)
        .bind(stored_reason)
        .bind(bounds.stored)
        .bind(update.claim_token)
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
    /// A cap that is already lower is left alone, and the row must still hold
    /// the token this delivery claimed under.
    ///
    /// # Examples
    ///
    /// ```text
    /// let clamped = scheduler.clamp_terminal_cap(TerminalCapClamp {
    ///     task_id,
    ///     worker_id,
    ///     claim_token,
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
            claim_token,
            attempt,
        } = clamp;
        let bounds = AttemptBounds::for_delivery(attempt)?;
        let result = sqlx::query(
            "UPDATE apalis.jobs
             SET max_attempts = LEAST(COALESCE(max_attempts, $4), $4)
             WHERE id = $1 AND lock_by = $2 AND status = 'Running' AND attempts = $3
               AND metadata->>'wildside.retry.claim_token' = $5",
        )
        .bind(task_id)
        .bind(worker_id)
        .bind(bounds.stored)
        .bind(bounds.acknowledged)
        .bind(claim_token)
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
