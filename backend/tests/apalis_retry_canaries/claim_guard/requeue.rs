//! Canary for the delivery that loses its row to a dead-letter requeue.
//!
//! A fetched delivery can wait in the worker's buffer while the row is
//! retired and requeued. The requeue resets `attempts` to zero, and a later
//! fetch by the same worker name restores `lock_by`, so only the requeue
//! counter separates the stale delivery from the replacement. This canary
//! drives the fetch, the production acknowledgement that retires the row, the
//! production requeue, the replacement fetch, and both claims in sequence.

use apalis_core::error::BoxDynError;
use apalis_core::worker::context::WorkerContext;
use apalis_core::worker::ext::ack::Acknowledge;
use apalis_postgres::PgTask;
use backend::outbound::queue::AttemptBoundAck;
use serde_json::{Map, Value};
use tracing_test::traced_test;

use super::{CLAIM_CAP, claim_task, queue_for};
use crate::ack_guard::{ProbeFailure, requeue_dead_job};
use crate::fixtures::{
    CanaryEnvironment, Delivery, JobId, JobRow, QueueName, WorkerName, claim_due_rows,
    delivery_task, insert_job, read_fetched_metadata, read_state, register_queue_worker,
};
use crate::support::apalis_job_id::apalis_job_id;

/// The worker name the stale and the replacement delivery both fetch under.
const REQUEUE_WORKER: &str = "canary-claim-requeue-worker";

/// The attempt limit the stale delivery's row starts with.
///
/// The delivery spends this single attempt, so its acknowledgement retires the
/// row and leaves it for the operator to requeue.
const REQUEUE_FIRST_CAP: i32 = 1;

/// Metadata key the dead-letter requeue increments.
const REQUEUE_COUNT_KEY: &str = "wildside.retry.requeue_count";

/// Metadata key the lock step stamps with its claim token.
const CLAIM_TOKEN_KEY: &str = "wildside.retry.claim_token";

/// Register the worker and insert a due row that spends its only attempt.
fn insert_due_row(environment: &CanaryEnvironment, queue: QueueName<'_>, id: JobId<'_>) {
    environment.run(async {
        register_queue_worker(&environment.pool, WorkerName(REQUEUE_WORKER), queue).await;
        insert_job(
            &environment.pool,
            JobRow::new(queue.as_str(), id.as_str(), "Pending")
                .attempts(0)
                .max_attempts(REQUEUE_FIRST_CAP),
        )
        .await;
    });
}

/// Fetch one due row through the pinned fetcher and assert it was claimed.
fn fetch_one(environment: &CanaryEnvironment, queue: QueueName<'_>) {
    let fetched = claim_due_rows(environment, queue.as_str(), REQUEUE_WORKER);
    assert_eq!(fetched, 1, "the pinned fetcher claims exactly one due row");
}

/// Build the delivery that the fetched metadata describes.
///
/// A delivery carries the metadata object as the fetch returned it, so the
/// requeue generation and any other observed value travel unchanged.
fn fetched_delivery(
    id: JobId<'_>,
    max_attempts: i32,
    metadata: &Map<String, Value>,
) -> PgTask<Value> {
    let mut task = delivery_task(
        id.as_str(),
        Delivery::new(REQUEUE_WORKER, max_attempts, 0).fetched(metadata),
    );
    task.parts
        .data
        .insert(WorkerContext::new::<Value>(REQUEUE_WORKER));
    task
}

/// Claim the stale delivery and return the token its claim stamped.
fn claim_stale_delivery(environment: &CanaryEnvironment, task: &PgTask<Value>) -> String {
    let claimed =
        claim_task(environment, task.clone()).expect("the delivery that fetched the row claims it");
    claimed
        .meta()
        .get(CLAIM_TOKEN_KEY)
        .and_then(Value::as_str)
        .expect("the claim stamps its token on the delivery context")
        .to_owned()
}

/// Retire the row the stale delivery owns through the production
/// acknowledgement.
///
/// A requeue accepts only a dead row, while a fetch accepts only a row that is
/// `Pending` or a retryable `Failed`, so a row cannot be requeued straight
/// after it was fetched. The row has to be retired in between, and only the
/// delivery holding the lock can do that. This step is therefore the stale
/// delivery reaching its attempt cap: it claims, is acknowledged as killed, and
/// then replays that same claim after the requeue. The claim under test is that
/// replay, so what it observes is what the fetch captured before the requeue.
fn retire_row(environment: &CanaryEnvironment, id: JobId<'_>, token: &str) {
    let acknowledged = environment.run(async {
        let task = delivery_task(
            id.as_str(),
            Delivery::new(REQUEUE_WORKER, REQUEUE_FIRST_CAP, 1).claimed(token),
        );
        let result: Result<(), BoxDynError> = Err(Box::new(ProbeFailure));
        AttemptBoundAck::new(environment.pool.clone())
            .ack(&result, &task.parts)
            .await
    });
    acknowledged.expect("the terminal delivery is acknowledged as killed");
}

/// Assert the requeue advanced the counter and reset the attempt count.
fn assert_requeue_advanced(environment: &CanaryEnvironment, id: JobId<'_>) {
    let requeued = read_fetched_metadata(&environment.runtime, &environment.pool, id.as_str());
    assert_eq!(
        requeued.get(REQUEUE_COUNT_KEY),
        Some(&Value::from(1)),
        "the requeue advances the counter the replacement fetch will carry"
    );
    let state = read_state(&environment.runtime, &environment.pool, id.as_str());
    assert_eq!(state.attempts, 0, "the requeue resets the attempt count");
    assert_eq!(
        state.lock_by, None,
        "the requeue clears the recorded lock owner"
    );
}

/// Assert the replacement fetch restored the discriminators the stale delivery
/// carries.
fn assert_replacement_fetch_matches(environment: &CanaryEnvironment, id: JobId<'_>) {
    let state = read_state(&environment.runtime, &environment.pool, id.as_str());
    assert_eq!(
        state.status, "Queued",
        "the replacement fetch queues the requeued row"
    );
    assert_eq!(
        state.attempts, 0,
        "the requeue reset the count to the value the stale delivery carries"
    );
    assert_eq!(
        state.lock_by.as_deref(),
        Some(REQUEUE_WORKER),
        "the replacement fetch restored the lock owner the stale delivery names"
    );
}

/// Assert the replacement's claim promoted the row under its own worker.
fn assert_claimed(environment: &CanaryEnvironment, id: JobId<'_>) {
    let state = read_state(&environment.runtime, &environment.pool, id.as_str());
    assert_eq!(
        state.status, "Running",
        "the replacement's claim promotes the row"
    );
    assert_eq!(
        state.lock_by.as_deref(),
        Some(REQUEUE_WORKER),
        "the replacement's claim records its own worker"
    );
}

/// Scenario: a delivery fetched at attempt zero before a dead-letter requeue,
/// and a replacement fetched afterwards under the same worker name.
///
/// Invariant: the stale delivery's claim matches no row even though its
/// identifier, worker name, status, and attempt count all match the
/// replacement's fetched row, because the requeue advanced the row's counter
/// past the value the stale fetch captured. The replacement's own claim still
/// succeeds. Without the counter predicate the stale claim would promote the
/// row and mint its own token, and the replacement would then lose its claim.
#[traced_test]
#[test]
fn a_delivery_fetched_before_a_requeue_cannot_claim_the_replacement() {
    let environment = CanaryEnvironment::shared();
    let id = apalis_job_id();
    let queue_name = queue_for(&id);
    let queue = QueueName(queue_name.as_str());

    let job = JobId(id.as_str());

    insert_due_row(environment, queue, job);
    fetch_one(environment, queue);
    let stale_metadata = read_fetched_metadata(&environment.runtime, &environment.pool, &id);
    assert!(
        !stale_metadata.contains_key(REQUEUE_COUNT_KEY),
        "the stale delivery is fetched before any requeue counter exists: {stale_metadata:?}"
    );
    let stale_task = fetched_delivery(job, REQUEUE_FIRST_CAP, &stale_metadata);
    let token = claim_stale_delivery(environment, &stale_task);
    retire_row(environment, job, &token);

    // The operator requeues the retired row, which resets the attempt count
    // and advances the counter the stale delivery never saw.
    requeue_dead_job(environment, queue, job);
    assert_requeue_advanced(environment, job);

    // The same worker name fetches the replacement, so every discriminator the
    // old predicate compared now matches the stale delivery again.
    fetch_one(environment, queue);
    assert_replacement_fetch_matches(environment, job);
    let replacement_metadata = read_fetched_metadata(&environment.runtime, &environment.pool, &id);
    let before = read_state(&environment.runtime, &environment.pool, &id);
    let replacement_task = fetched_delivery(job, CLAIM_CAP, &replacement_metadata);

    let stale = claim_task(environment, stale_task);
    assert!(
        stale.is_err(),
        "the stale delivery must not claim the replacement's row: {stale:?}"
    );
    assert!(
        logs_contain("claim_refused"),
        "a refused claim must log its decision, because the caller only sees a missing row"
    );
    assert_eq!(
        read_state(&environment.runtime, &environment.pool, &id),
        before,
        "the refused claim leaves the replacement's fetched row untouched"
    );

    claim_task(environment, replacement_task)
        .expect("the replacement delivery claims the row its fetch returned");
    assert_claimed(environment, job);
}
