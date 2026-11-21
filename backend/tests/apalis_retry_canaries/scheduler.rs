//! PostgreSQL canaries for the retry scheduler's ownership guard.
//!
//! The retry wrapper proves ownership before it asks the scheduler to write,
//! but the guarded `UPDATE` is the statement that actually prevents a stale
//! write when a row is completed, re-locked, or delivered again between those
//! two steps. These canaries drive `PostgresRetryScheduler` directly so the SQL
//! predicate, and not only the wrapper's precondition, is pinned against a real
//! `apalis.jobs` table. The cases that exercise a refused write, and the
//! terminal-cap fallback, live in [`guards`]; the ones that drive the real
//! acknowledgement and fetch after that fallback live in [`terminated`].

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use backend::outbound::queue::{
    ClockSource, PostgresRetryScheduler, RetryPersistence, RetryScheduleUpdate,
    TerminalRetryReason, TerminalRetryUpdate,
};
use chrono::{DateTime, TimeZone, Utc};
use serde_json::Value;
use sqlx::PgPool;

use super::fixtures::{CanaryEnvironment, JobRow, insert_job, register_worker};
use crate::support::apalis_job_id::apalis_job_id;

// The parent module is loaded through `#[path]`, which stops Rust from
// deriving a nested directory for its children, so the path is explicit here.
#[path = "scheduler/guards.rs"]
mod guards;

#[path = "scheduler/terminated.rs"]
mod terminated;

/// The retry delay the happy-path canary schedules.
const RETRY_DELAY: Duration = Duration::from_secs(30);

/// Tolerated drift between the scheduling statement and the reading statement.
///
/// The update evaluates `now()` when it runs and the read evaluates it again
/// later, so an exact comparison would measure the gap between the two
/// statements rather than the delay the scheduler stored.
const CLOCK_SLACK_MICROS: i64 = 5_000_000;

/// A row's deadline as microseconds since the Unix epoch.
const DEADLINE_SQL: &str =
    "SELECT (extract(epoch FROM run_at) * 1000000)::bigint FROM apalis.jobs WHERE id = $1";

/// A row's deadline as microseconds from the database's own clock.
///
/// Expressing the deadline relative to `now()` in the same statement keeps the
/// assertion independent of the application's clock.
const DEADLINE_OFFSET_SQL: &str =
    "SELECT (extract(epoch FROM run_at - now()) * 1000000)::bigint FROM apalis.jobs WHERE id = $1";

/// The worker that owns the rows these canaries insert.
const OWNER: &str = "canary-scheduler-owner";

/// A worker that holds a lock the caller does not.
const OTHER_WORKER: &str = "canary-scheduler-other-worker";

/// A worker that attempts a write it does not own.
const CALLER: &str = "canary-scheduler-caller";

/// Attempts the ordinary canary row has recorded before this delivery.
///
/// Apalis increments its in-memory attempt on the first poll, so a row holding
/// [`STORED_ATTEMPTS`] is handed to a handler that counts
/// [`DELIVERED_ATTEMPT`]. The guarded writes match the stored value, which is
/// what binds them to this delivery.
const STORED_ATTEMPTS: i32 = 2;

/// The attempt the canary's delivery counts.
const DELIVERED_ATTEMPT: u32 = 3;

/// The attempt the canary's delivery reports, as the scheduler receives it.
fn delivery_attempt() -> NonZeroU32 {
    NonZeroU32::new(DELIVERED_ATTEMPT).expect("the delivered attempt is non-zero")
}

/// The queue, worker, status, and counters that describe one canary row.
struct SchedulerRow<'a> {
    queue: &'a str,
    worker: &'a str,
    status: &'a str,
    attempts: i32,
    max_attempts: i32,
    /// When the row becomes eligible, in seconds from now; negative is due.
    run_at_offset_seconds: f64,
}

impl<'a> SchedulerRow<'a> {
    /// A running row owned by `worker`, with the ordinary attempt counters.
    ///
    /// Its deadline is in the future, so the scheduling canaries can prove a
    /// write moved it. A canary that needs the fetcher to consider the row
    /// calls [`due`](Self::due).
    fn running(queue: &'a str, worker: &'a str) -> Self {
        Self {
            queue,
            worker,
            status: "Running",
            attempts: STORED_ATTEMPTS,
            max_attempts: 5,
            run_at_offset_seconds: 3_600.0,
        }
    }

    /// Make the row already eligible, so a fetch would claim it.
    fn due(mut self) -> Self {
        self.run_at_offset_seconds = -1.0;
        self
    }
}

/// Insert a row that the named canary worker may or may not own.
///
/// `metadata` seeds unrelated keys so a terminal write can prove it preserved
/// them; the ordinary scheduler canaries pass `None`.
async fn insert_scheduler_row(
    pool: &PgPool,
    id: &str,
    row: SchedulerRow<'_>,
    metadata: Option<Value>,
) {
    register_worker(pool, row.worker).await;
    let mut job = JobRow::new(row.queue, id, row.status)
        .attempts(row.attempts)
        .max_attempts(row.max_attempts)
        .run_at_offset_seconds(row.run_at_offset_seconds)
        .locked_by(row.worker);
    if let Some(metadata) = metadata {
        job = job.metadata(metadata);
    }
    insert_job(pool, job).await;
}

/// A canary row together with the environment and pool that hold it.
struct Canary {
    environment: &'static CanaryEnvironment,
    pool: PgPool,
    id: String,
}

impl Canary {
    /// Insert a row and return the handles that drive it.
    fn start(row: SchedulerRow<'_>, metadata: Option<Value>) -> Self {
        let environment = CanaryEnvironment::shared();
        let pool = environment.pool.clone();
        let id = apalis_job_id();
        environment.run(insert_scheduler_row(&pool, &id, row, metadata));
        Self {
            environment,
            pool,
            id,
        }
    }

    /// Read one `bigint` column for this row, naming it in any failure.
    fn read_micros(&self, sql: &str, what: &str) -> i64 {
        // Build the message eagerly so `expect` still names the column; a
        // closure that panics is what the workspace lint forbids.
        let failure = format!("read canary {what}");
        self.environment.runtime.block_on(async {
            let (value,): (i64,) = sqlx::query_as(sql)
                .bind(&self.id)
                .fetch_one(&self.pool)
                .await
                .expect(&failure);
            value
        })
    }

    /// Read the attempt cap and metadata a terminal write would rewrite.
    fn read_terminal_state(&self) -> TerminalState {
        self.environment.runtime.block_on(async {
            let (max_attempts, metadata): (i32, Option<String>) = sqlx::query_as(
                "SELECT max_attempts, metadata::text FROM apalis.jobs WHERE id = $1",
            )
            .bind(&self.id)
            .fetch_one(&self.pool)
            .await
            .expect("read canary terminal state");
            TerminalState {
                max_attempts,
                metadata,
            }
        })
    }

    /// Assert a guarded write reports no update and leaves the row unchanged.
    ///
    /// Reading the row twice, rather than trusting the boolean alone, is what
    /// proves a refused write did not persist a partial change.
    fn assert_write_is_refused<T: PartialEq + std::fmt::Debug>(
        &self,
        read: impl Fn(&Self) -> T,
        write: impl FnOnce() -> bool,
        reason: &str,
    ) {
        let before = read(self);
        let updated = write();

        assert!(!updated, "{reason}");
        assert_eq!(
            read(self),
            before,
            "the guarded update must leave the row untouched"
        );
    }
}

/// The stored state a terminal write may change.
#[derive(Debug, PartialEq, Eq)]
struct TerminalState {
    max_attempts: i32,
    metadata: Option<String>,
}

/// Build the terminal update a canary worker would persist.
fn terminal_update<'a>(
    id: &'a str,
    worker: &'a str,
    reason: TerminalRetryReason,
    shrink_limit: bool,
) -> TerminalRetryUpdate<'a> {
    TerminalRetryUpdate {
        task_id: id,
        worker_id: worker,
        attempt: delivery_attempt(),
        reason,
        shrink_limit,
    }
}

#[test]
fn scheduler_advances_the_deadline_for_an_owned_running_row() {
    let canary = Canary::start(SchedulerRow::running("canary.scheduler-owned", OWNER), None);
    let before = canary.read_micros(DEADLINE_OFFSET_SQL, "deadline offset");

    let updated = canary
        .environment
        .run(
            PostgresRetryScheduler::new(canary.pool.clone()).schedule_retry(RetryScheduleUpdate {
                task_id: &canary.id,
                worker_id: OWNER,
                attempt: delivery_attempt(),
                delay: RETRY_DELAY,
            }),
        )
        .expect("the guarded update executes");

    assert!(updated, "the recorded lock owner still owns a running row");
    let expected = i64::try_from(RETRY_DELAY.as_micros()).expect("delay fits in i64");
    let after = canary.read_micros(DEADLINE_OFFSET_SQL, "deadline offset");
    assert!(
        before > expected + CLOCK_SLACK_MICROS,
        "the fixture's original deadline must be further away, saw {before}us"
    );
    assert!(
        (expected - CLOCK_SLACK_MICROS..=expected + CLOCK_SLACK_MICROS).contains(&after),
        "the deadline must move to the requested delay from now, saw {after}us"
    );
}

/// A clock that always reports the same instant.
struct FixedClock(DateTime<Utc>);

impl ClockSource for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        self.0
    }
}

/// Scenario: the scheduler runs with an injected clock.
///
/// Invariant: the persisted deadline is exactly that instant plus the delay,
/// so a fixed clock makes the stored value a property of the input rather
/// than of when the statement happened to run.
#[test]
fn scheduler_persists_the_injected_instant_plus_the_delay() {
    let instant = Utc
        .with_ymd_and_hms(2031, 5, 6, 7, 8, 9)
        .single()
        .expect("the fixed instant is unambiguous");
    let canary = Canary::start(SchedulerRow::running("canary.scheduler-clock", OWNER), None);
    let scheduler =
        PostgresRetryScheduler::new(canary.pool.clone()).with_clock(Arc::new(FixedClock(instant)));

    let updated = canary
        .environment
        .run(scheduler.schedule_retry(RetryScheduleUpdate {
            task_id: &canary.id,
            worker_id: OWNER,
            attempt: delivery_attempt(),
            delay: RETRY_DELAY,
        }))
        .expect("the guarded update executes");

    assert!(updated, "the recorded lock owner still owns a running row");
    let delay = i64::try_from(RETRY_DELAY.as_micros()).expect("delay fits in i64");
    assert_eq!(
        canary.read_micros(DEADLINE_SQL, "deadline"),
        instant.timestamp_micros() + delay,
        "the persisted deadline must be the injected instant plus the delay"
    );
}

#[test]
fn terminal_write_records_the_reason_for_an_owned_running_row() {
    let canary = Canary::start(
        SchedulerRow::running("canary.terminal-owned", OWNER),
        Some(serde_json::json!({ "keep": "marker" })),
    );

    let recorded = canary
        .environment
        .run(
            PostgresRetryScheduler::new(canary.pool.clone()).record_terminal_failure(
                terminal_update(&canary.id, OWNER, TerminalRetryReason::Exhausted, true),
            ),
        )
        .expect("the guarded update executes");

    assert!(recorded, "the recorded lock owner still owns a running row");
    let after = canary.read_terminal_state();
    assert_eq!(
        after.max_attempts, 3,
        "a shrinking terminal write stores the delivered attempt as the new cap"
    );
    let stored: Value = serde_json::from_str(after.metadata.as_deref().expect("metadata retained"))
        .expect("metadata remains valid JSON");
    assert_eq!(
        stored["wildside.retry.dead_reason"],
        serde_json::json!("exhausted")
    );
    assert_eq!(
        stored["keep"],
        serde_json::json!("marker"),
        "the terminal write must preserve unrelated metadata keys"
    );
}
