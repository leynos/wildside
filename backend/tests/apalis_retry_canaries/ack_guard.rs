//! Canaries for the attempt-bound acknowledgement.
//!
//! The pinned acknowledgement matches a delivery on `id` and `lock_by`, so
//! distinct worker names keep one instance's stale delivery away from another
//! instance's row. They cannot separate two deliveries of the same instance:
//! orphan recovery returns the row to `Pending`, and the worker that lost it can
//! claim it again under its own name while the original handler is still
//! running. [`AttemptBoundAck`] adds the attempt count the row stored during the
//! delivery to its predicate, which is the column that distinguishes the two
//! generations.
//!
//! These canaries pin both directions with the worker name held equal: the stale
//! generation changes nothing, and the generation that owns the row is still
//! acknowledged.

use std::error::Error as StdError;

use apalis_core::error::BoxDynError;
use apalis_core::worker::ext::ack::Acknowledge;
use backend::outbound::queue::AttemptBoundAck;

use super::fixtures::{
    CanaryEnvironment, JobRow, insert_job, locked_task, read_state, register_worker,
};
use crate::support::apalis_job_id::apalis_job_id;

/// The worker name both deliveries register under.
const SHARED_WORKER: &str = "canary-ack-guard-shared";

/// The attempt count the row stores after recovery returned it to the worker.
///
/// The stale delivery ran while the row stored one less; recovery incremented
/// that count, so the same number now identifies the replacement generation's
/// starting point rather than the stale delivery's.
const RECOVERED_ATTEMPTS: i32 = 5;

/// The queue holding the re-locked replacement row.
const GUARD_QUEUE: &str = "canary.ack-guard";

/// Insert a `Running` row under the shared worker name, as the replacement
/// delivery's lock step leaves it.
fn insert_locked_row(environment: &CanaryEnvironment, queue: &str, id: &str) {
    let pool = environment.pool.clone();
    environment.run(async {
        register_worker(&pool, SHARED_WORKER).await;
        insert_job(
            &pool,
            JobRow::new(queue, id, "Running")
                .attempts(RECOVERED_ATTEMPTS)
                .max_attempts(10)
                .run_at_offset_seconds(3_600.0)
                .locked_by(SHARED_WORKER),
        )
        .await;
    });
}

/// Acknowledging a delivery that reports the attempt count the row held before
/// recovery changed it, under the name the replacement also uses.
fn acknowledge_attempt(
    environment: &CanaryEnvironment,
    id: &str,
    attempt: usize,
) -> Result<(), sqlx::Error> {
    environment.run(async {
        let task = locked_task(id, SHARED_WORKER, 10, attempt);
        let result: Result<(), BoxDynError> = Err(Box::new(ProbeFailure));
        AttemptBoundAck::new(environment.pool.clone())
            .ack(&result, &task.parts)
            .await
    })
}

/// Scenario: the delivery that lost the row acknowledges after the same worker
/// name re-claimed it.
///
/// Invariant: the acknowledgement matches no row and reports that, so the
/// replacement's `Running` state, attempt count, and lock owner survive. The
/// worker name alone cannot establish this — only the stored attempt count can,
/// which is the guard this canary exists for.
#[test]
fn a_stale_acknowledgement_cannot_overwrite_a_same_name_replacement() {
    let environment = CanaryEnvironment::shared();
    let pool = environment.pool.clone();
    let id = apalis_job_id();

    insert_locked_row(environment, GUARD_QUEUE, &id);
    let before = read_state(&environment.runtime, &pool, &id);

    // The stale delivery reported the count the row held while it owned it,
    // one below the count recovery left behind.
    let acknowledged = acknowledge_attempt(environment, &id, RECOVERED_ATTEMPTS as usize);

    assert!(
        matches!(acknowledged, Err(sqlx::Error::RowNotFound)),
        "a stale acknowledgement must report the row it could not match: {acknowledged:?}"
    );
    assert_eq!(
        read_state(&environment.runtime, &pool, &id),
        before,
        "the replacement keeps its Running status, attempts, and lock owner"
    );
}

/// Scenario: the delivery that owns the row acknowledges it under the shared
/// worker name.
///
/// Invariant: the acknowledgement succeeds and stores the delivered attempt, so
/// the guard refuses only stale generations rather than every acknowledgement a
/// re-used worker name produces.
#[test]
fn the_owning_delivery_still_acknowledges_its_same_name_row() {
    let environment = CanaryEnvironment::shared();
    let pool = environment.pool.clone();
    let id = apalis_job_id();

    insert_locked_row(environment, GUARD_QUEUE, &id);
    let acknowledged = acknowledge_attempt(environment, &id, RECOVERED_ATTEMPTS as usize + 1);

    acknowledged.expect("the delivery that owns the row acknowledges it");
    let state = read_state(&environment.runtime, &pool, &id);
    assert_eq!(
        state.status, "Failed",
        "a failure below the cap stays Failed"
    );
    assert_eq!(
        state.attempts,
        RECOVERED_ATTEMPTS + 1,
        "the acknowledgement stores the delivered attempt"
    );
    assert_eq!(
        state.lock_by.as_deref(),
        Some(SHARED_WORKER),
        "the acknowledgement keeps the recorded lock owner"
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
