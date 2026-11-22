//! Canaries for the generation-bound acknowledgement.
//!
//! The pinned acknowledgement matches a delivery on `id` and `lock_by`, so
//! distinct worker names keep one instance's stale delivery away from another
//! instance's row. They cannot separate two deliveries of the same instance:
//! orphan recovery returns the row to `Pending`, and the worker that lost it can
//! claim it again under its own name while the original handler is still
//! running. [`AttemptBoundAck`] adds a claim token, which the lock step rewrites
//! on every claim, and the attempt count the row stored during the delivery.
//!
//! The attempt count alone leaves a window open, because a dead-letter requeue
//! resets it to zero. The requeue canary pins that window with every other
//! discriminator equal: the identifiers, worker name, `Running` status, and
//! stored attempt count all match, and only the token separates the generations.
//! It drives the production dead-letter adapter's `requeue`, the production lock
//! step, and the acknowledgement in sequence, because the window only exists
//! while a replacement delivery is in flight; a real worker would have
//! acknowledged before the stale delivery could be replayed, and the stored
//! status would then differ for a reason unrelated to the token.
//!
//! Every canary holds the worker name equal in both directions: the stale
//! generation changes nothing, and the generation that owns the row is still
//! acknowledged.

use std::error::Error as StdError;
use std::future::Ready;
use std::num::NonZeroU32;
use std::task::{Context, Poll};
use std::time::Duration;

use apalis_core::error::BoxDynError;
use apalis_core::layers::{Layer, Service};
use apalis_core::worker::context::WorkerContext;
use apalis_core::worker::ext::ack::Acknowledge;
use apalis_postgres::{PgContext, PgTask};
use backend::domain::jobs::retry::{JobRetryPolicy, JobRetryPolicyConfig};
use backend::domain::ports::{DeadJobId, DeadLetterQueue};
use backend::outbound::queue::{
    AttemptBoundAck, AttemptBoundLock, PostgresDeadLetterQueue, RequeueBudgets,
};
use serde_json::Value;

use super::fixtures::{
    CanaryEnvironment, ClaimToken, Delivery, JobId, JobRow, QueueName, claim_due_rows,
    delivery_task, insert_job, read_state, register_worker,
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

/// The attempt cap that lets a delivery below it fail without being retired.
const GUARD_CAP: i32 = 10;

/// The attempt count the requeue canary's terminal row stores.
///
/// The requeue resets it, so it only has to be a value a terminal write leaves
/// behind; it is not what separates the two generations.
const DEAD_ATTEMPTS: i32 = 2;

/// The queue holding the re-locked replacement row.
const GUARD_QUEUE: &str = "canary.ack-guard";

/// The claim token the generation that owns the row stamped on it.
const REPLACEMENT_CLAIM: &str = "canary-ack-replacement-claim";

/// The claim token the stale delivery carries from its own claim.
const STALE_CLAIM: &str = "canary-ack-stale-claim";

/// A `Running` row as the replacement delivery's lock step leaves it.
///
/// The lock step writes the claim token, so a replacement that is still
/// running carries that token and no dead reason.
fn locked_row<'a>(queue: QueueName<'a>, id: JobId<'a>, attempts: i32) -> JobRow<'a> {
    JobRow::new(queue.as_str(), id.as_str(), "Running")
        .attempts(attempts)
        .max_attempts(GUARD_CAP)
        .run_at_offset_seconds(3_600.0)
        .metadata(serde_json::json!({
            "wildside.retry.claim_token": REPLACEMENT_CLAIM
        }))
        .locked_by(SHARED_WORKER)
}

/// The terminal row a replacement leaves before an operator requeues it.
///
/// The replacement claimed the row under [`REPLACEMENT_CLAIM`] before it ran,
/// and neither its acknowledgement nor the later requeue clears that token, so
/// the token is the only part of the row that still identifies the generation.
fn dead_row<'a>(queue: QueueName<'a>, id: JobId<'a>, attempts: i32) -> JobRow<'a> {
    JobRow::new(queue.as_str(), id.as_str(), "Killed")
        .attempts(attempts)
        .max_attempts(GUARD_CAP)
        .metadata(serde_json::json!({
            "wildside.retry.claim_token": REPLACEMENT_CLAIM,
            "wildside.retry.dead_reason": "exhausted"
        }))
        .locked_by(SHARED_WORKER)
}

/// Insert one builder row under the shared worker name.
fn insert_row(environment: &CanaryEnvironment, row: JobRow<'_>) {
    let pool = environment.pool.clone();
    environment.run(async {
        register_worker(&pool, SHARED_WORKER).await;
        insert_job(&pool, row).await;
    });
}

/// Acknowledge a delivery reporting one attempt count under one claim token.
fn acknowledge_attempt(
    environment: &CanaryEnvironment,
    id: JobId<'_>,
    attempt: usize,
    claim: ClaimToken<'_>,
) -> Result<(), sqlx::Error> {
    environment.run(async {
        let task = delivery_task(
            id.as_str(),
            Delivery::new(SHARED_WORKER, GUARD_CAP, attempt).claimed(claim.as_str()),
        );
        let result: Result<(), BoxDynError> = Err(Box::new(ProbeFailure));
        AttemptBoundAck::new(environment.pool.clone())
            .ack(&result, &task.parts)
            .await
    })
}

/// Requeue one dead row through the production dead-letter adapter.
///
/// The budget is the guard cap, so the restored row is claimable and the
/// replacement delivery below it fails rather than retiring.
pub(super) fn requeue_dead_job(
    environment: &CanaryEnvironment,
    queue: QueueName<'_>,
    id: JobId<'_>,
) {
    let policy = JobRetryPolicy::new(JobRetryPolicyConfig {
        max_attempts: NonZeroU32::new(u32::try_from(GUARD_CAP).expect("positive guard cap"))
            .expect("non-zero guard cap"),
        base_delay: Duration::from_millis(10),
        multiplier: 2.0,
        max_delay: Duration::from_millis(80),
        jitter_fraction: 0.0,
    })
    .expect("valid canary retry policy");
    let dead_letters = PostgresDeadLetterQueue::new(
        environment.pool.clone(),
        RequeueBudgets::from_policies([(queue.as_str(), policy)]),
    );
    environment
        .run(dead_letters.requeue(&DeadJobId::new(id.as_str())))
        .expect("the dead job is requeued");
}

/// Claim a requeued row through the production lock step under `worker`.
///
/// Returns the token the claim stamped, which the replacement delivery carries
/// into its acknowledgement. Driving the lock directly rather than a worker
/// keeps the replacement in flight: a real worker would acknowledge before the
/// stale delivery could be replayed, and the stored status would then differ
/// for a reason unrelated to the token.
///
/// The claim is bound to the fetch that queued the row, so the canary performs
/// that fetch first with the pinned backend's own function. The delivery then
/// carries the attempt count the row holds before it is polled, which is the
/// count the claim compares against the stored one.
pub(super) fn claim_row(
    environment: &CanaryEnvironment,
    worker: &str,
    queue: QueueName<'_>,
    id: JobId<'_>,
) -> String {
    let claimed = claim_due_rows(environment, queue.as_str(), worker);
    assert_eq!(
        claimed, 1,
        "the pinned fetcher queues exactly the requeued row"
    );
    let mut service = AttemptBoundLock::new(environment.pool.clone()).layer(ContextProbe);
    let ctx = environment
        .run(async {
            let mut task = delivery_task(id.as_str(), Delivery::new(worker, GUARD_CAP, 0));
            // A delivery reaches the lock inside a worker, which has already
            // put its own context in the task's extensions.
            task.parts.data.insert(WorkerContext::new::<Value>(worker));
            service.call(task).await
        })
        .expect("the lock step claims the requeued row");
    ctx.meta()
        .get("wildside.retry.claim_token")
        .and_then(Value::as_str)
        .expect("the claim stamps its token on the delivery context")
        .to_owned()
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

/// Scenario: the delivery that lost the row acknowledges after the same worker
/// name re-claimed it.
///
/// Invariant: the acknowledgement matches no row and reports that, so the
/// replacement's `Running` state, attempt count, and lock owner survive. The
/// worker name and attempt count cannot establish this on their own — recovery
/// left the count one higher, and the claim token differs — which is the guard
/// this canary exists for.
#[test]
fn a_stale_acknowledgement_cannot_overwrite_a_same_name_replacement() {
    let environment = CanaryEnvironment::shared();
    let pool = environment.pool.clone();
    let id = apalis_job_id();

    insert_row(
        environment,
        locked_row(QueueName(GUARD_QUEUE), JobId(&id), RECOVERED_ATTEMPTS),
    );
    let before = read_state(&environment.runtime, &pool, &id);

    // The stale delivery reported the count the row held while it owned it,
    // one below the count recovery left behind.
    let acknowledged = acknowledge_attempt(
        environment,
        JobId(&id),
        RECOVERED_ATTEMPTS as usize,
        ClaimToken(STALE_CLAIM),
    );

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

    insert_row(
        environment,
        locked_row(QueueName(GUARD_QUEUE), JobId(&id), RECOVERED_ATTEMPTS),
    );
    let acknowledged = acknowledge_attempt(
        environment,
        JobId(&id),
        RECOVERED_ATTEMPTS as usize + 1,
        ClaimToken(REPLACEMENT_CLAIM),
    );

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

/// Scenario: a replacement delivery ran to terminal, an operator requeued the
/// row, and the same worker claimed it again while the original delivery was
/// still active.
///
/// Invariant: the original delivery's acknowledgement matches no row even
/// though its identifier, worker name, `Running` status, and stored attempt
/// count all match, because the claim token it carries belongs to the
/// generation the requeue replaced. The request that owns the row still
/// acknowledges it. Without the token this canary would overwrite the new
/// delivery's row, which is why the attempt count alone is not a generation.
#[test]
fn a_requeued_row_cannot_be_overwritten_by_its_previous_delivery() {
    let environment = CanaryEnvironment::shared();
    let pool = environment.pool.clone();
    let id = apalis_job_id();

    insert_row(
        environment,
        dead_row(QueueName(GUARD_QUEUE), JobId(&id), DEAD_ATTEMPTS),
    );
    requeue_dead_job(environment, QueueName(GUARD_QUEUE), JobId(&id));

    let requeued = read_state(&environment.runtime, &pool, &id);
    assert_eq!(
        requeued.status, "Pending",
        "a requeue returns the row to the fetcher"
    );
    assert_eq!(
        requeued.attempts, 0,
        "a requeue resets the attempt count, which is why it cannot identify a generation"
    );
    assert_eq!(
        requeued.claim_token.as_deref(),
        Some(REPLACEMENT_CLAIM),
        "a requeue preserves the token of the generation it replaced"
    );

    let token = claim_row(
        environment,
        SHARED_WORKER,
        QueueName(GUARD_QUEUE),
        JobId(&id),
    );
    let claim = ClaimToken(token.as_str());
    let running = read_state(&environment.runtime, &pool, &id);
    assert_eq!(running.status, "Running", "the lock step promotes the row");
    assert_eq!(
        running.claim_token.as_deref(),
        Some(claim.as_str()),
        "the row now carries the token the new claim stamped"
    );
    assert_ne!(
        claim.as_str(),
        REPLACEMENT_CLAIM,
        "every claim mints a token that never repeats"
    );

    // Both generations report attempt 1 against a row storing zero, so the
    // token is the only discriminator left.
    let stale = acknowledge_attempt(environment, JobId(&id), 1, ClaimToken(STALE_CLAIM));

    assert!(
        matches!(stale, Err(sqlx::Error::RowNotFound)),
        "the previous generation must not match the re-claimed row: {stale:?}"
    );
    assert_eq!(
        read_state(&environment.runtime, &pool, &id),
        running,
        "the re-claimed row keeps its Running status, attempts, and claim token"
    );

    acknowledge_attempt(environment, JobId(&id), 1, claim)
        .expect("the generation that claimed the row acknowledges it");
    let state = read_state(&environment.runtime, &pool, &id);
    assert_eq!(
        state.status, "Failed",
        "a failed delivery under a wide cap stays Failed"
    );
    assert_eq!(
        state.attempts, 1,
        "the acknowledgement stores the delivered attempt"
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
