//! Unit checks for the range guards and the injected retry clock.

use super::{
    AttemptBounds, ClockSource, PostgresRetryScheduler, RetryScheduleUpdate, TerminalCapClamp,
    TerminalRetryReason, TerminalRetryUpdate, retry_deadline,
};
use chrono::{DateTime, TimeZone, Utc};
use sqlx::Error as SqlxError;
use sqlx::postgres::PgPoolOptions;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

/// A delivery attempt that cannot be zero.
fn attempt(value: u32) -> NonZeroU32 {
    NonZeroU32::new(value).expect("the test attempt is non-zero")
}

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

/// The retry update a range guard can reject before it reaches a server.
fn schedule_update(delay: Duration) -> RetryScheduleUpdate<'static> {
    RetryScheduleUpdate {
        task_id: "task",
        worker_id: "worker",
        attempt: attempt(2),
        delay,
    }
}

#[tokio::test]
async fn schedule_retry_rejects_a_delay_beyond_the_interval_range() {
    let scheduler = scheduler_without_server();

    let error = scheduler
        .schedule_retry(schedule_update(Duration::from_secs(u64::MAX)))
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
        attempt: attempt(u32::MAX),
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

#[tokio::test]
async fn clamp_terminal_cap_rejects_an_attempt_beyond_the_integer_range() {
    let scheduler = scheduler_without_server();
    let clamp = TerminalCapClamp {
        task_id: "task",
        worker_id: "worker",
        attempt: attempt(u32::MAX),
    };

    let error = scheduler
        .clamp_terminal_cap(clamp)
        .await
        .expect_err("the attempt cannot be stored in an i32 column");

    assert!(
        matches!(&error, SqlxError::Protocol(message)
            if message.contains("retry attempt exceeds")),
        "expected a protocol error about the retry attempt, got {error:?}"
    );
}

#[test]
fn attempt_bounds_hold_the_stored_and_acknowledged_counts() {
    let bounds = AttemptBounds::for_delivery(attempt(3)).expect("three fits in an i32");

    assert_eq!(
        (bounds.stored, bounds.acknowledged),
        (2, 3),
        "the row holds one less than the delivery counts until the acknowledgement lands"
    );
}

#[test]
fn attempt_bounds_accept_the_first_delivery() {
    let bounds = AttemptBounds::for_delivery(attempt(1)).expect("one fits in an i32");

    assert_eq!(
        (bounds.stored, bounds.acknowledged),
        (0, 1),
        "a first delivery finds a row that has never been attempted"
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
        .schedule_retry(schedule_update(Duration::from_secs(1)))
        .await
        .expect_err("the lazy pool cannot connect");

    assert!(
        !matches!(&error, SqlxError::Protocol(_)),
        "the deadline computation must succeed and the failure must come from the connection attempt, got {error:?}"
    );
}
