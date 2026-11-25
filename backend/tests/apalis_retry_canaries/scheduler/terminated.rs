//! Canaries for a terminated delivery whose terminal write failed.
//!
//! A terminal classification can fail to persist after the wrapper has already
//! decided the job must not run again. The cap-lowering fallback is then the
//! only thing between that decision and Apalis acknowledging the delivery as a
//! `Failed` row the fetcher would claim. These canaries drive the real
//! acknowledgement and the real fetch, so the protection is pinned as
//! behaviour rather than only as a column value.

use std::error::Error as StdError;
use std::time::Duration;

use apalis_core::error::BoxDynError;
use apalis_core::worker::ext::ack::Acknowledge;
use apalis_postgres::PgAck;

use super::{CLAIM_TOKEN, Canary, DELIVERED_ATTEMPT, OWNER, SchedulerRow, delivery_attempt};
use crate::fixtures::{Delivery, claim_due_rows, delivery_task, fetch_one_task};
use backend::domain::ports::DeadReason;
use backend::outbound::queue::{
    PostgresRetryScheduler, RetryPersistence, TerminalCapClamp, TerminalRetryUpdate,
};

/// The attempt limit the delivery was fetched with.
///
/// Apalis decides an acknowledgement's status from the task context it cloned
/// before the handler ran, so it compares the delivered attempt against this
/// stale value and cannot see the cap the fallback lowered.
const FETCHED_MAX_ATTEMPTS: i32 = 5;

/// A queue whose only row is the terminated delivery.
const PROTECTED_QUEUE: &str = "canary.terminated-protected";

/// A queue whose row is acknowledged without the fallback.
const UNPROTECTED_QUEUE: &str = "canary.terminated-unprotected";

/// A queue whose fallback cannot reach a database.
const UNREACHABLE_QUEUE: &str = "canary.terminated-unreachable";

/// The worker that owns the row acknowledged without the fallback.
///
/// The fetching canaries each name their own worker because the pinned backend
/// takes a session advisory lock while it registers a worker, so two fetches
/// under one name would refuse each other when they land on different pooled
/// connections. A worker of its own keeps each canary independent of the other.
const UNPROTECTED_OWNER: &str = "canary-terminated-unprotected-owner";

/// The worker that owns the row whose fallback cannot reach the database.
const UNREACHABLE_OWNER: &str = "canary-terminated-unreachable-owner";

/// A scheduler whose database refuses every connection.
///
/// Port 1 refuses immediately, so the write fails deterministically instead of
/// waiting on a timeout. The pool is built inside the canary runtime because
/// `connect_lazy` registers its idle reaper immediately.
fn unreachable_scheduler(canary: &Canary) -> PostgresRetryScheduler {
    canary.environment.run(async {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(Duration::from_millis(500))
            .connect_lazy("postgres://wildside:wildside@127.0.0.1:1/wildside")
            .expect("build a lazily connected pool for an unreachable database");
        PostgresRetryScheduler::new(pool)
    })
}

/// Run the terminal write the wrapper attempts before its fallback.
///
/// This is the write whose failure the fallback exists to cover, so driving it
/// against the unreachable database keeps the canary faithful to the sequence
/// the handler performs instead of starting at the fallback.
fn terminal_write_fails(canary: &Canary, scheduler: &PostgresRetryScheduler) -> bool {
    canary.environment.run(async {
        scheduler
            .record_terminal_failure(TerminalRetryUpdate {
                task_id: &canary.id,
                worker_id: OWNER,
                claim_token: CLAIM_TOKEN,
                attempt: delivery_attempt(),
                reason: DeadReason::Fatal,
                shrink_limit: true,
            })
            .await
            .is_err()
    })
}

/// Acknowledge the delivery as Apalis does after the handler reports a failure.
fn acknowledge_failure(canary: &Canary, worker: &str) {
    let attempt = usize::try_from(DELIVERED_ATTEMPT).expect("the delivered attempt is a usize");
    canary.environment.run(async {
        let task = delivery_task(
            &canary.id,
            Delivery::new(worker, FETCHED_MAX_ATTEMPTS, attempt),
        );
        let result: Result<(), BoxDynError> = Err(Box::new(ProbeFailure));

        PgAck::new(canary.pool.clone())
            .ack(&result, &task.parts)
            .await
            .expect("acknowledge the terminated delivery");
    });
}

/// The status, attempt count, and attempt limit the row now holds.
fn read_outcome(canary: &Canary) -> (String, i32, i32) {
    canary.environment.run(async {
        sqlx::query_as("SELECT status, attempts, max_attempts FROM apalis.jobs WHERE id = $1")
            .bind(&canary.id)
            .fetch_one(&canary.pool)
            .await
            .expect("read the acknowledged canary row")
    })
}

/// Whether the dead-letter listing selects the row.
///
/// The predicate is repeated here rather than imported because this canary
/// asserts an operator-visible outcome, not the adapter's implementation.
fn is_dead(canary: &Canary) -> bool {
    canary.environment.run(async {
        sqlx::query_scalar(
            "SELECT EXISTS (
                 SELECT 1 FROM apalis.jobs
                 WHERE id = $1
                   AND (status = 'Killed'
                        OR (status = 'Failed'
                            AND (max_attempts IS NULL OR attempts >= max_attempts)))
             )",
        )
        .bind(&canary.id)
        .fetch_one(&canary.pool)
        .await
        .expect("read the dead-letter predicate for the canary row")
    })
}

/// Run the fallback the wrapper performs after a failed terminal write.
fn clamp_cap(canary: &Canary) -> bool {
    canary
        .environment
        .run(
            PostgresRetryScheduler::new(canary.pool.clone()).clamp_terminal_cap(TerminalCapClamp {
                task_id: &canary.id,
                worker_id: OWNER,
                claim_token: CLAIM_TOKEN,
                attempt: delivery_attempt(),
            }),
        )
        .expect("the fallback executes")
}

/// Scenario: a terminal write failed, so the wrapper lowered the cap instead.
///
/// Invariant: the acknowledgement still stores the delivery as failed, the
/// fetcher does not claim it again, and the dead-letter listing still shows it,
/// so an operator can inspect and requeue the job.
#[test]
fn a_terminated_delivery_is_not_fetched_after_acknowledgement() {
    let canary = Canary::start(SchedulerRow::running(PROTECTED_QUEUE, OWNER).due(), None);

    assert!(
        clamp_cap(&canary),
        "the recorded lock owner still owns a running row"
    );
    acknowledge_failure(&canary, OWNER);

    let attempt = i32::try_from(DELIVERED_ATTEMPT).expect("the delivered attempt is an i32");
    assert_eq!(
        read_outcome(&canary),
        ("Failed".to_owned(), attempt, attempt),
        "the acknowledgement stores a failed row whose cap is the delivered attempt"
    );
    assert_eq!(
        claim_due_rows(canary.environment, PROTECTED_QUEUE, OWNER),
        0,
        "a failed row at its attempt cap must not be delivered again"
    );
    assert!(
        is_dead(&canary),
        "the protected row must stay visible to the dead-letter listing"
    );
}

/// Scenario: the same delivery is acknowledged without the fallback.
///
/// Invariant: the fetcher claims the row again. This is the defect the fallback
/// exists to prevent, and asserting it is what gives the other canary teeth.
#[test]
fn without_the_fallback_the_acknowledged_row_is_fetched_again() {
    let canary = Canary::start(
        SchedulerRow::running(UNPROTECTED_QUEUE, UNPROTECTED_OWNER).due(),
        None,
    );

    acknowledge_failure(&canary, UNPROTECTED_OWNER);

    let fetched = fetch_one_task(canary.environment, UNPROTECTED_QUEUE, UNPROTECTED_OWNER);
    assert_eq!(
        fetched.task_id, canary.id,
        "the fetcher must claim the row the fallback did not protect"
    );
}

/// Scenario: the terminal write failed and the fallback cannot reach the
/// database either.
///
/// Invariant: the acknowledgement leaves a failed row that the fetcher claims
/// again. This is the residual window the `terminal_cap_clamp_exhausted` alarm
/// reports, and asserting it keeps that alarm honest instead of implying the
/// wrapper can always protect the row.
#[test]
fn a_fallback_that_never_lands_leaves_the_row_fetchable() {
    let canary = Canary::start(
        SchedulerRow::running(UNREACHABLE_QUEUE, UNREACHABLE_OWNER).due(),
        None,
    );
    let scheduler = unreachable_scheduler(&canary);

    assert!(
        terminal_write_fails(&canary, &scheduler),
        "an unreachable database must report the terminal write as failed"
    );
    let clamp = canary
        .environment
        .run(scheduler.clamp_terminal_cap(TerminalCapClamp {
            task_id: &canary.id,
            worker_id: OWNER,
            claim_token: CLAIM_TOKEN,
            attempt: delivery_attempt(),
        }));
    assert!(
        clamp.is_err(),
        "an unreachable database must report the fallback as failed, not applied"
    );

    acknowledge_failure(&canary, UNREACHABLE_OWNER);

    let delivered = i32::try_from(DELIVERED_ATTEMPT).expect("the delivered attempt is an i32");
    assert_eq!(
        read_outcome(&canary),
        ("Failed".to_owned(), delivered, FETCHED_MAX_ATTEMPTS),
        "a fallback that never lands leaves the fetched attempt cap in place"
    );
    let fetched = fetch_one_task(canary.environment, UNREACHABLE_QUEUE, UNREACHABLE_OWNER);
    assert_eq!(
        fetched.task_id, canary.id,
        "the residual window hands the retired job back to the fetcher"
    );
}

/// The failure an acknowledged delivery reports.
#[derive(Debug)]
struct ProbeFailure;

impl std::fmt::Display for ProbeFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("probe failure")
    }
}

impl StdError for ProbeFailure {}
