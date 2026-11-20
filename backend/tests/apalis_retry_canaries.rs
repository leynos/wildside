//! Version-coupling canaries for durable retry scheduling on Apalis PostgreSQL.
//!
//! The retry wrapper depends on behaviour owned by `apalis-postgres` and its
//! `apalis.get_jobs` SQL function. Those are implementation details of a
//! release candidate, so this suite pins the behaviour the wrapper relies on:
//!
//! - the scheduling columns exist and keep their names;
//! - only due `Failed` rows with spare attempts are fetched;
//! - `Killed` rows and rows at their attempt cap are never fetched;
//! - a fetched task keeps its persisted attempt count;
//! - acknowledgement leaves `run_at`, `max_attempts`, and `metadata` intact;
//!   and
//! - the retry wrapper writes nothing unless the recorded lock owner matches
//!   the worker that delivered the task.
//!
//! A failure here means an Apalis upgrade changed the contract, not that the
//! wrapper regressed. The database fixtures and the two fetch helpers live in
//! [`fixtures`], so each module below reads as assertions about the contract.

include!("support/entrypoint.rs");
declare_test_support!(
    apalis_job_id,
    atexit_cleanup,
    embedded_postgres,
    queue_retry_worker
);

#[path = "apalis_retry_canaries/ack.rs"]
mod ack;
#[path = "apalis_retry_canaries/ack_guard.rs"]
mod ack_guard;
#[path = "apalis_retry_canaries/fixtures.rs"]
mod fixtures;
#[path = "apalis_retry_canaries/identity.rs"]
mod identity;
#[path = "apalis_retry_canaries/pipeline.rs"]
mod pipeline;
#[path = "apalis_retry_canaries/recovery.rs"]
mod recovery;
#[path = "apalis_retry_canaries/scheduler.rs"]
mod scheduler;

use self::fixtures::{
    CanaryEnvironment, JobRow, RowState, claim_due_rows, fetch_one_task, insert_job, read_state,
    register_worker,
};
use self::support::apalis_job_id::apalis_job_id;

#[test]
fn scheduling_columns_exist() {
    let environment = CanaryEnvironment::shared();
    let columns: Vec<String> = environment
        .run(async {
            sqlx::query_scalar(
                "SELECT column_name::text
                 FROM information_schema.columns
                 WHERE table_schema = 'apalis' AND table_name = 'jobs'",
            )
            .fetch_all(&environment.pool)
            .await
        })
        .expect("read apalis.jobs columns");

    for required in ["run_at", "max_attempts", "metadata", "attempts", "status"] {
        assert!(
            columns.iter().any(|column| column == required),
            "apalis.jobs must keep its {required} column; found {columns:?}"
        );
    }
}

#[test]
fn failed_row_with_spare_attempts_is_fetched_when_due() {
    let environment = CanaryEnvironment::shared();
    let pool = environment.pool.clone();
    let queue = "canary.due";
    let worker = "canary-due-worker";
    let id = apalis_job_id();

    environment.run(async {
        register_worker(&pool, worker).await;
        insert_job(&pool, JobRow::new(queue, &id, "Failed")).await;
    });

    let fetched = fetch_one_task(environment, queue, worker);
    assert_eq!(fetched.task_id, id);

    // Proving the claim landed keeps this positive case from passing on a
    // fetch that never touched the row. The fetcher stops at the queue step,
    // so the row is `Queued` rather than the `Running` the worker's own lock
    // step would set.
    let state = read_state(&environment.runtime, &pool, &id);
    assert_eq!(
        state.status, "Queued",
        "fetch queues the row for the worker"
    );
    assert_eq!(state.lock_by.as_deref(), Some(worker));
}

/// A row the fetcher must overlook, with the reason it is ineligible.
struct IneligibleRow {
    queue: &'static str,
    worker: &'static str,
    status: &'static str,
    attempts: i32,
    run_at_offset_seconds: f64,
    expectation: &'static str,
}

#[rstest::rstest]
#[case::run_at_in_the_future(IneligibleRow {
    queue: "canary.future",
    worker: "canary-future-worker",
    status: "Failed",
    attempts: 1,
    run_at_offset_seconds: 3_600.0,
    expectation: "a row scheduled in the future must not be fetched",
})]
#[case::killed_row(IneligibleRow {
    queue: "canary.killed",
    worker: "canary-killed-worker",
    status: "Killed",
    attempts: 1,
    run_at_offset_seconds: -1.0,
    expectation: "a Killed row must never be fetched again",
})]
#[case::attempt_cap_reached(IneligibleRow {
    queue: "canary.exhausted",
    worker: "canary-exhausted-worker",
    status: "Failed",
    attempts: 5,
    run_at_offset_seconds: -1.0,
    expectation: "a Failed row at its attempt cap must not be fetched",
})]
fn ineligible_rows_are_not_fetched(#[case] case: IneligibleRow) {
    let environment = CanaryEnvironment::shared();
    let pool = environment.pool.clone();
    let id = apalis_job_id();
    let row = JobRow::new(case.queue, &id, case.status)
        .attempts(case.attempts)
        .run_at_offset_seconds(case.run_at_offset_seconds);

    environment.run(async {
        register_worker(&pool, case.worker).await;
        insert_job(&pool, row).await;
    });

    // The direct call completes or fails, so a zero count is evidence that the
    // predicate refused the row. Watching the backend stream instead would
    // read the same on a fetch that never completed.
    let claimed = claim_due_rows(environment, case.queue, case.worker);
    assert_eq!(claimed, 0, "{}", case.expectation);
    assert_eq!(
        read_state(&environment.runtime, &pool, &id),
        RowState {
            status: case.status.to_owned(),
            attempts: case.attempts,
            lock_by: None,
        },
        "{} must stay unclaimed",
        case.status
    );
}

#[test]
fn fetch_preserves_the_persisted_attempt_count() {
    let environment = CanaryEnvironment::shared();
    let pool = environment.pool.clone();
    let queue = "canary.attempts";
    let worker = "canary-attempts-worker";
    let id = apalis_job_id();

    environment.run(async {
        register_worker(&pool, worker).await;
        insert_job(
            &pool,
            JobRow::new(queue, &id, "Failed")
                .attempts(3)
                .max_attempts(10),
        )
        .await;
    });

    let fetched = fetch_one_task(environment, queue, worker);
    assert_eq!(
        fetched.attempt, 3,
        "the wrapper must see the persisted attempt count, not a reset one"
    );
    assert_eq!(fetched.max_attempts, 10);
}
