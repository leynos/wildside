//! Canaries for the fetch-bound claim step.
//!
//! The lock step runs on a task the worker fetched earlier, and the row can
//! move on while that task waits in the worker's buffer. Orphan recovery
//! returns the row to `Pending`, clears `lock_by`, and increments `attempts`;
//! a dead-letter requeue clears `lock_by` and resets `attempts` to zero. Either
//! way another worker can fetch the row first. [`AttemptBoundLock`] therefore
//! claims a row only while it still names the fetching worker and still holds
//! the attempt count the delivery was built with.
//!
//! Both canaries drive the pinned fetcher, the production step that moves the
//! row on, and the production lock step in sequence, because the window only
//! exists between a fetch and the claim that follows it. Each ends with the
//! replacement delivery claiming the row, so the guard is shown to refuse only
//! a delivery that lost its row.

use std::future::Ready;
use std::task::{Context, Poll};

use apalis_core::error::BoxDynError;
use apalis_core::layers::{Layer, Service};
use apalis_core::worker::context::WorkerContext;
use apalis_postgres::{PgContext, PgTask};
use backend::outbound::queue::AttemptBoundLock;
use serde_json::Value;

use super::ack_guard::requeue_dead_job;
use super::fixtures::{
    CanaryEnvironment, Delivery, JobId, JobRow, QueueName, WorkerName, claim_due_rows,
    delivery_task, insert_job, mark_worker_stale, read_state, recover_orphaned_row,
    register_queue_worker,
};
use crate::support::apalis_job_id::apalis_job_id;

/// The attempt cap that lets a delivery fail without being retired.
const CLAIM_CAP: i32 = 10;

/// The delivery that fetched the row and then stalled.
const STALE_WORKER: &str = "canary-claim-stale";

/// The delivery that fetches the row after it has moved on.
const REPLACEMENT_WORKER: &str = "canary-claim-replacement";

/// The worker whose heartbeat drives Apalis's recovery sweep.
const SWEEPER_WORKER: &str = "canary-claim-sweeper";

/// A queue name unique to one canary's row.
///
/// The fetch claims one due row per queue, so a shared queue name would let one
/// canary's fetch take another's row when the two run concurrently.
fn queue_for(id: &str) -> String {
    format!("canary.claim-guard-{id}")
}

/// The row the stalled delivery fetched, before it was queued.
fn pending_row<'a>(queue: &'a str, id: &'a str) -> JobRow<'a> {
    JobRow::new(queue, id, "Pending")
        .attempts(0)
        .max_attempts(CLAIM_CAP)
}

/// A terminal row an operator requeues while its delivery is still active.
fn dead_row<'a>(queue: &'a str, id: &'a str) -> JobRow<'a> {
    JobRow::new(queue, id, "Killed")
        .attempts(2)
        .max_attempts(CLAIM_CAP)
        .metadata(serde_json::json!({
            "wildside.retry.claim_token": "canary-claim-previous",
            "wildside.retry.dead_reason": "exhausted"
        }))
        .locked_by(STALE_WORKER)
}

/// Run the production lock step for one delivery and report its outcome.
fn claim_delivery(
    environment: &CanaryEnvironment,
    worker: &str,
    id: &str,
    fetched_attempt: usize,
) -> Result<PgContext, BoxDynError> {
    let mut service = AttemptBoundLock::new(environment.pool.clone()).layer(ContextProbe);
    environment.run(async {
        let mut task = delivery_task(id, Delivery::new(worker, CLAIM_CAP, fetched_attempt));
        // A delivery reaches the lock inside a worker, which has already put
        // its own context in the task's extensions.
        task.parts.data.insert(WorkerContext::new::<Value>(worker));
        service.call(task).await
    })
}

/// Inner service that hands back the context the lock layer stamped.
///
/// The lock is the only production behaviour under test here, so the wrapped
/// service does nothing except expose the mutated context to the canary.
#[derive(Clone, Copy)]
struct ContextProbe;

impl Service<PgTask<Value>> for ContextProbe {
    type Response = PgContext;
    type Error = BoxDynError;
    type Future = Ready<Result<PgContext, BoxDynError>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: PgTask<Value>) -> Self::Future {
        std::future::ready(Ok(req.parts.ctx))
    }
}

/// Scenario: the delivery fetched the row, stalled, and orphan recovery handed
/// the row to another worker.
///
/// Invariant: the stalled delivery's claim matches no row, so its handler is
/// never polled, and the replacement's claim still succeeds. Recovery
/// incremented the attempt count and cleared the lock owner, so either bound
/// predicate alone would refuse the obsolete delivery here; a claim that
/// checked neither would take the row the replacement had just fetched.
#[test]
fn a_stale_delivery_cannot_claim_a_recovered_row() {
    let environment = CanaryEnvironment::shared();
    let id = apalis_job_id();
    let queue_name = queue_for(&id);
    let queue = QueueName(queue_name.as_str());
    environment.run(async {
        register_queue_worker(&environment.pool, WorkerName(STALE_WORKER), queue).await;
        register_queue_worker(&environment.pool, WorkerName(REPLACEMENT_WORKER), queue).await;
        insert_job(&environment.pool, pending_row(queue_name.as_str(), &id)).await;
    });

    let stalled_fetch = claim_due_rows(environment, queue_name.as_str(), STALE_WORKER);
    assert_eq!(stalled_fetch, 1, "the stalled delivery fetched the row");

    environment.run(mark_worker_stale(&environment.pool, STALE_WORKER));
    recover_orphaned_row(environment, queue, WorkerName(SWEEPER_WORKER), JobId(&id));
    let recovered = read_state(&environment.runtime, &environment.pool, &id);
    assert_eq!(
        recovered.status, "Pending",
        "orphan recovery returns the row to the fetcher"
    );
    assert_eq!(
        recovered.attempts, 1,
        "orphan recovery increments the attempt count"
    );
    assert_eq!(
        recovered.lock_by, None,
        "orphan recovery clears the recorded lock owner"
    );

    let replacement_fetch = claim_due_rows(environment, queue_name.as_str(), REPLACEMENT_WORKER);
    assert_eq!(
        replacement_fetch, 1,
        "the replacement fetches the recovered row"
    );
    let before = read_state(&environment.runtime, &environment.pool, &id);

    let stale = claim_delivery(environment, STALE_WORKER, &id, 0);
    assert!(
        stale.is_err(),
        "the stalled delivery must not claim the recovered row: {stale:?}"
    );
    assert_eq!(
        read_state(&environment.runtime, &environment.pool, &id),
        before,
        "the refused claim leaves the replacement's row untouched"
    );

    claim_delivery(environment, REPLACEMENT_WORKER, &id, 1)
        .expect("the delivery that fetched the row still claims it");
    let claimed = read_state(&environment.runtime, &environment.pool, &id);
    assert_eq!(
        claimed.status, "Running",
        "the fresh claim promotes the row"
    );
    assert_eq!(
        claimed.lock_by.as_deref(),
        Some(REPLACEMENT_WORKER),
        "the fresh claim records its own worker"
    );
}

/// Scenario: an operator requeued the row while the delivery that had owned it
/// was still active, which reset the attempt count to zero.
///
/// Invariant: the previous delivery's claim matches no row even though its
/// attempt count now equals the row's, because the requeue cleared the lock
/// owner. A claim bound to the attempt count alone would take the row before
/// its replacement could fetch it.
#[test]
fn a_requeued_row_cannot_be_claimed_by_its_previous_delivery() {
    let environment = CanaryEnvironment::shared();
    let id = apalis_job_id();
    let queue_name = queue_for(&id);
    let queue = QueueName(queue_name.as_str());
    environment.run(async {
        register_queue_worker(&environment.pool, WorkerName(STALE_WORKER), queue).await;
        register_queue_worker(&environment.pool, WorkerName(REPLACEMENT_WORKER), queue).await;
        insert_job(&environment.pool, dead_row(queue_name.as_str(), &id)).await;
    });

    requeue_dead_job(environment, queue, JobId(&id));
    let requeued = read_state(&environment.runtime, &environment.pool, &id);
    assert_eq!(
        requeued.status, "Pending",
        "a requeue returns the row to the fetcher"
    );
    assert_eq!(
        requeued.attempts, 0,
        "a requeue resets the attempt count, so it cannot refuse this claim"
    );
    assert_eq!(
        requeued.lock_by, None,
        "a requeue clears the recorded lock owner"
    );

    let stale = claim_delivery(environment, STALE_WORKER, &id, 0);
    assert!(
        stale.is_err(),
        "the previous delivery must not claim the requeued row: {stale:?}"
    );
    assert_eq!(
        read_state(&environment.runtime, &environment.pool, &id),
        requeued,
        "the refused claim leaves the requeued row untouched"
    );

    let replacement_fetch = claim_due_rows(environment, queue_name.as_str(), REPLACEMENT_WORKER);
    assert_eq!(
        replacement_fetch, 1,
        "the replacement fetches the requeued row"
    );
    claim_delivery(environment, REPLACEMENT_WORKER, &id, 0)
        .expect("the delivery that fetched the requeued row claims it");
    assert_eq!(
        read_state(&environment.runtime, &environment.pool, &id).status,
        "Running",
        "the fresh claim promotes the requeued row"
    );
}
