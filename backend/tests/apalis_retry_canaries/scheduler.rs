//! PostgreSQL canaries for the retry scheduler's ownership guard.
//!
//! The retry wrapper proves ownership before it asks the scheduler to write,
//! but the guarded `UPDATE` is the statement that actually prevents a stale
//! write when a row is completed or re-locked between those two steps. These
//! canaries drive `PostgresRetryScheduler` directly so the SQL predicate, and
//! not only the wrapper's precondition, is pinned against a real
//! `apalis.jobs` table.

use std::time::Duration;

use backend::outbound::queue::PostgresRetryScheduler;
use sqlx::PgPool;

use super::{CanaryEnvironment, JobRow, insert_job, register_worker};
use crate::support::apalis_job_id::apalis_job_id;

/// The retry delay the happy-path canary schedules.
const RETRY_DELAY: Duration = Duration::from_secs(30);

/// Tolerated drift between the scheduling statement and the reading statement.
///
/// The update evaluates `now()` when it runs and the read evaluates it again
/// later, so an exact comparison would measure the gap between the two
/// statements rather than the delay the scheduler stored.
const CLOCK_SLACK_MICROS: i64 = 5_000_000;

/// Read a row's next-deadline as microseconds since the Unix epoch.
fn read_deadline_micros(runtime: &tokio::runtime::Runtime, pool: &PgPool, id: &str) -> i64 {
    runtime.block_on(async {
        let (deadline,): (i64,) = sqlx::query_as(
            "SELECT (extract(epoch FROM run_at) * 1000000)::bigint
             FROM apalis.jobs WHERE id = $1",
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("read canary deadline");
        deadline
    })
}

/// Read a row's deadline as microseconds from the database's own clock.
///
/// Expressing the deadline relative to `now()` in the same statement keeps the
/// assertion independent of the application's clock.
fn read_deadline_offset_micros(runtime: &tokio::runtime::Runtime, pool: &PgPool, id: &str) -> i64 {
    runtime.block_on(async {
        let (offset,): (i64,) = sqlx::query_as(
            "SELECT (extract(epoch FROM run_at - now()) * 1000000)::bigint
             FROM apalis.jobs WHERE id = $1",
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("read canary deadline offset");
        offset
    })
}

/// A scheduler canary row, identified by the worker that holds its lock.
struct SchedulerRow<'a> {
    queue: &'a str,
    worker: &'a str,
    status: &'a str,
    id: &'a str,
}

/// Insert a row that the named canary worker may or may not own.
async fn insert_scheduler_row(pool: &PgPool, row: SchedulerRow<'_>) {
    register_worker(pool, row.worker).await;
    insert_job(
        pool,
        JobRow::new(row.queue, row.id, row.status)
            .attempts(1)
            .max_attempts(5)
            .run_at_offset_seconds(3_600.0)
            .locked_by(row.worker),
    )
    .await;
}

#[test]
fn scheduler_refuses_a_row_that_is_not_running() {
    let environment = CanaryEnvironment::shared();
    let pool = environment.pool.clone();
    let queue = "canary.scheduler-completed";
    let worker = "canary-scheduler-completed-worker";
    let id = apalis_job_id();

    environment.run(insert_scheduler_row(
        &pool,
        SchedulerRow {
            queue,
            worker,
            status: "Done",
            id: &id,
        },
    ));
    let before = read_deadline_micros(&environment.runtime, &pool, &id);

    let updated = environment
        .run(PostgresRetryScheduler::new(pool.clone()).schedule_retry(&id, worker, RETRY_DELAY))
        .expect("the guarded update executes");

    assert!(!updated, "a completed row must not accept a retry deadline");
    assert_eq!(
        read_deadline_micros(&environment.runtime, &pool, &id),
        before,
        "the guarded update must leave a completed row untouched"
    );
}

#[test]
fn scheduler_refuses_a_row_owned_by_another_worker() {
    let environment = CanaryEnvironment::shared();
    let pool = environment.pool.clone();
    let queue = "canary.scheduler-foreign-lock";
    let owner = "canary-scheduler-foreign-owner";
    let caller = "canary-scheduler-foreign-caller";
    let id = apalis_job_id();

    environment.run(insert_scheduler_row(
        &pool,
        SchedulerRow {
            queue,
            worker: owner,
            status: "Running",
            id: &id,
        },
    ));
    let before = read_deadline_micros(&environment.runtime, &pool, &id);

    let updated = environment
        .run(PostgresRetryScheduler::new(pool.clone()).schedule_retry(&id, caller, RETRY_DELAY))
        .expect("the guarded update executes");

    assert!(
        !updated,
        "only the recorded lock owner may move the deadline"
    );
    assert_eq!(
        read_deadline_micros(&environment.runtime, &pool, &id),
        before,
        "the guarded update must leave another worker's row untouched"
    );
}

#[test]
fn scheduler_advances_the_deadline_for_an_owned_running_row() {
    let environment = CanaryEnvironment::shared();
    let pool = environment.pool.clone();
    let queue = "canary.scheduler-owned";
    let worker = "canary-scheduler-owned-worker";
    let id = apalis_job_id();

    environment.run(insert_scheduler_row(
        &pool,
        SchedulerRow {
            queue,
            worker,
            status: "Running",
            id: &id,
        },
    ));
    let before = read_deadline_offset_micros(&environment.runtime, &pool, &id);

    let updated = environment
        .run(PostgresRetryScheduler::new(pool.clone()).schedule_retry(&id, worker, RETRY_DELAY))
        .expect("the guarded update executes");

    assert!(updated, "the recorded lock owner still owns a running row");
    let expected = i64::try_from(RETRY_DELAY.as_micros()).expect("delay fits in i64");
    let after = read_deadline_offset_micros(&environment.runtime, &pool, &id);
    assert!(
        before > expected + CLOCK_SLACK_MICROS,
        "the fixture's original deadline must be further away, saw {before}us"
    );
    assert!(
        (expected - CLOCK_SLACK_MICROS..=expected + CLOCK_SLACK_MICROS).contains(&after),
        "the deadline must move to the requested delay from now, saw {after}us"
    );
}
