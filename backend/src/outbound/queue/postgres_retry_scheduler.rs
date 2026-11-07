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
    ///     .schedule_retry(task_id, worker_id, delay)
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
    /// Returns `false` when the row is no longer owned by this worker or is not
    /// running. The deadline is taken from the injected [`ClockSource`] and
    /// bound as the new `run_at`.
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
    /// executed, or a protocol error when the delay is not representable as a
    /// microsecond offset or the resulting deadline lies outside the supported
    /// timestamp range.
    #[tracing::instrument(
        skip_all,
        fields(
            operation = "schedule_retry",
            task_id,
            worker_id,
            delay_micros = %delay.as_micros(),
            scheduled_at = tracing::field::Empty,
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
        let scheduled_at = retry_deadline(self.clock.as_ref(), delay_micros)?;
        tracing::Span::current().record("scheduled_at", tracing::field::display(scheduled_at));

        let result = sqlx::query(
            "UPDATE apalis.jobs
             SET run_at = $3
             WHERE id = $1 AND lock_by = $2 AND status = 'Running'",
        )
        .bind(task_id)
        .bind(worker_id)
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
mod tests {
    //! Unit checks for the range guards and the injected retry clock.

    use super::{
        ClockSource, PostgresRetryScheduler, TerminalRetryReason, TerminalRetryUpdate,
        retry_deadline,
    };
    use chrono::{DateTime, TimeZone, Utc};
    use sqlx::Error as SqlxError;
    use sqlx::postgres::PgPoolOptions;
    use std::num::NonZeroU32;
    use std::sync::Arc;
    use std::time::Duration;

    /// Clock fixed at one instant so the deadline the scheduler writes is known.
    #[derive(Debug, Clone, Copy)]
    struct FixedClock(DateTime<Utc>);

    impl ClockSource for FixedClock {
        fn now(&self) -> DateTime<Utc> {
            self.0
        }
    }

    fn fixed_clock() -> FixedClock {
        FixedClock(
            Utc.with_ymd_and_hms(2030, 1, 1, 0, 0, 0)
                .single()
                .expect("the fixed instant is unambiguous"),
        )
    }

    /// Build a scheduler whose pool never opens a connection.
    ///
    /// The range guards under test run before the query is executed, so a
    /// lazily connected pool keeps them fast and independent of a server.
    fn scheduler_without_server() -> PostgresRetryScheduler {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://wildside@127.0.0.1:1/wildside")
            .expect("lazy pool parses");
        PostgresRetryScheduler::new(pool)
    }

    #[tokio::test]
    async fn schedule_retry_rejects_a_delay_beyond_the_interval_range() {
        let scheduler = scheduler_without_server();

        let error = scheduler
            .schedule_retry("task", "worker", Duration::from_secs(u64::MAX))
            .await
            .expect_err("the delay cannot be expressed in microseconds as i64");

        assert!(
            matches!(&error, SqlxError::Protocol(message)
                if message.contains("retry delay exceeds")),
            "expected a protocol error about the retry delay, got {error:?}"
        );
    }

    #[tokio::test]
    async fn record_terminal_failure_rejects_an_attempt_beyond_the_integer_range() {
        let scheduler = scheduler_without_server();
        let update = TerminalRetryUpdate {
            task_id: "task",
            worker_id: "worker",
            attempt: NonZeroU32::new(u32::MAX).expect("u32::MAX is non-zero"),
            reason: TerminalRetryReason::Exhausted,
            shrink_limit: true,
        };

        let error = scheduler
            .record_terminal_failure(update)
            .await
            .expect_err("the attempt cannot be stored in an i32 column");

        assert!(
            matches!(&error, SqlxError::Protocol(message)
                if message.contains("retry attempt exceeds")),
            "expected a protocol error about the retry attempt, got {error:?}"
        );
    }

    #[test]
    fn retry_deadline_offsets_the_clock_instant_by_the_delay() {
        let clock = fixed_clock();

        let deadline = retry_deadline(&clock, 1_500_000).expect("the deadline is representable");

        assert_eq!(
            deadline,
            clock.0 + chrono::Duration::milliseconds(1_500),
            "the deadline must be the injected instant plus the delay"
        );
    }

    #[tokio::test]
    async fn schedule_retry_computes_the_deadline_before_contacting_the_database() {
        // A short acquire timeout keeps the unreachable-server probe fast; the
        // deadline is computed before the pool is asked for a connection.
        let pool = PgPoolOptions::new()
            .acquire_timeout(Duration::from_millis(100))
            .connect_lazy("postgres://wildside@127.0.0.1:1/wildside")
            .expect("lazy pool parses");
        let scheduler = PostgresRetryScheduler::new(pool).with_clock(Arc::new(fixed_clock()));

        let error = scheduler
            .schedule_retry("task", "worker", Duration::from_secs(1))
            .await
            .expect_err("the lazy pool cannot connect");

        assert!(
            !matches!(&error, SqlxError::Protocol(_)),
            "the deadline computation must succeed and the failure must come from the connection attempt, got {error:?}"
        );
    }
}
