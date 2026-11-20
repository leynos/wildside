//! Canaries for the retired-row window through the real worker pipeline.
//!
//! The `recovery` module hands a fetched task straight to the wrapper, which
//! bypasses two facts of the worker pipeline: the in-memory attempt is
//! incremented on the first poll, and an acknowledgement follows every service
//! call. These canaries run a real `WorkerBuilder` worker over the same
//! attempt-bound storage the composition uses, so the row that a terminal write
//! and orphan recovery leave behind is delivered, refused, and acknowledged the
//! way a deployed worker handles it.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use backend::domain::jobs::retry::{JobFailure, JobRetryPolicy, JobRetryPolicyConfig};
use backend::domain::ports::NoOpRouteQueueMetrics;
use backend::outbound::queue::{
    PostgresRetryScheduler, RetryPersistence, TerminalRetryReason, TerminalRetryUpdate,
};
use sqlx::PgPool;

use super::fixtures::{
    CanaryEnvironment, JobId, JobRow, QueueName, RowState, WorkerName, insert_job,
    mark_worker_stale, read_state, recover_orphaned_row, register_queue_worker,
};
use super::support::queue_retry_worker::{
    DEFAULT_WORKER_DEADLINE, QueueRetryProbeJob, QueueRetryWorkerConfig, run_queue_retry_worker,
    unique_worker_name,
};
use crate::support::apalis_job_id::apalis_job_id;

/// The attempt cap a terminal write leaves on a retired row.
const RETIRED_CAP: i32 = 5;

/// The attempt count orphan recovery leaves on a row retired at the cap.
///
/// The terminating delivery ran with the row holding four attempts; recovery
/// increments that stored count, so the recovered row holds the cap itself.
const RECOVERED_ATTEMPTS: i32 = RETIRED_CAP;

/// Build the bounded worker configuration for one canary row.
fn worker_config(pool: &PgPool, queue: &str, probe_id: &str) -> QueueRetryWorkerConfig {
    QueueRetryWorkerConfig {
        pool: pool.clone(),
        queue_name: queue.to_owned(),
        worker_name: unique_worker_name(),
        probe_id: probe_id.to_owned(),
        policy: policy(),
        metrics: Arc::new(NoOpRouteQueueMetrics),
        stop_after_poll_intervals: None,
        deadline: DEFAULT_WORKER_DEADLINE,
    }
}

fn policy() -> JobRetryPolicy {
    JobRetryPolicy::new(JobRetryPolicyConfig {
        max_attempts: NonZeroU32::new(3).expect("positive attempt limit"),
        base_delay: Duration::from_millis(10),
        multiplier: 2.0,
        max_delay: Duration::from_millis(80),
        jitter_fraction: 0.0,
    })
    .expect("valid canary policy")
}

/// A handler that succeeds without recording anything.
fn succeeding_handler(
    _job: QueueRetryProbeJob,
    _attempt: NonZeroU32,
) -> std::future::Ready<Result<(), JobFailure>> {
    std::future::ready(Ok(()))
}

/// Scenario: a terminal write succeeded, the worker was lost before its
/// acknowledgement, and orphan recovery returned the row to the fetcher.
///
/// Invariant: the recovered delivery is refused before the handler runs and the
/// acknowledgement records a `Killed` row at the delivered attempt. This is the
/// window CodeRabbit asked to see driven end to end: the row holds the cap
/// (not one past it), and the pipeline's own poll increments the delivery to
/// one more than the cap.
#[test]
fn the_worker_pipeline_refuses_a_recovered_terminal_row() {
    let environment = CanaryEnvironment::shared();
    let pool = environment.pool.clone();
    let id = apalis_job_id();
    let queue = "canary.worker-retired";

    environment.run(async {
        // The row a terminal write and orphan recovery leave behind: the cap
        // lowered to the terminating attempt, the terminal reason stored, and
        // the status returned to `Pending` with the attempt count incremented.
        insert_job(
            &pool,
            JobRow::new(queue, &id, "Pending")
                .attempts(RECOVERED_ATTEMPTS)
                .max_attempts(RETIRED_CAP)
                .metadata(serde_json::json!({
                    "wildside.retry.dead_reason": "exhausted"
                })),
        )
        .await;
    });

    let report = environment
        .run(run_queue_retry_worker(
            worker_config(&pool, queue, &id),
            succeeding_handler,
        ))
        .expect("the worker run completes within its deadline");

    assert!(
        report.reached_terminal,
        "the refused delivery is acknowledged into a terminal row"
    );
    assert_eq!(
        report.runs_at_terminal, 0,
        "a recovered row past its cap must not reach the handler"
    );
    let state = read_state(&environment.runtime, &pool, &id);
    assert_eq!(
        state.status, "Killed",
        "the acknowledgement records the delivery that never ran"
    );
    assert_eq!(
        state.attempts,
        RETIRED_CAP + 1,
        "the acknowledgement stores the attempt the delivery reported"
    );
}

/// The queue holding a row recovered after its terminal write.
const TERMINAL_WINDOW_QUEUE: &str = "canary.worker-terminal-window";

/// The worker that recorded the terminal failure and then stopped heartbeating.
const CRASHED_WORKER: &str = "canary-terminal-window-crashed";

/// The claim token the crashed delivery's lock step stamped on the row.
const CRASHED_CLAIM: &str = "canary-terminal-window-claim";

/// Drive the production terminal write for one delivery, as the wrapper does
/// when its policy retires the job.
fn record_terminal_failure(
    environment: &CanaryEnvironment,
    id: &str,
    worker: &str,
    attempt: u32,
) -> bool {
    let scheduler = PostgresRetryScheduler::new(environment.pool.clone());
    environment
        .run(scheduler.record_terminal_failure(TerminalRetryUpdate {
            task_id: id,
            worker_id: worker,
            attempt: NonZeroU32::new(attempt).expect("positive terminating attempt"),
            reason: TerminalRetryReason::Exhausted,
            shrink_limit: true,
        }))
        .expect("the terminal write completes")
}

/// The row the crashed delivery owned when its policy retired it.
///
/// Its `run_at` stays due: recovery does not move it, and the recovered row
/// must be fetchable for the window to be exercised.
fn crashed_row<'a>(queue: QueueName<'a>, id: JobId<'a>) -> JobRow<'a> {
    JobRow::new(queue.as_str(), id.as_str(), "Running")
        .attempts(RETIRED_CAP - 1)
        .max_attempts(RETIRED_CAP)
        .metadata(serde_json::json!({
            "wildside.retry.claim_token": CRASHED_CLAIM
        }))
        .locked_by(CRASHED_WORKER)
}

/// Seed the queue worker and the row its delivery owned.
fn seed_crashed_delivery(environment: &CanaryEnvironment, queue: QueueName<'_>, id: JobId<'_>) {
    let pool = environment.pool.clone();
    environment.run(async {
        register_queue_worker(&pool, WorkerName(CRASHED_WORKER), queue).await;
        insert_job(&pool, crashed_row(queue, id)).await;
    });
}

/// The row a terminal write leaves, and whether the write still owned it.
struct RetiredDelivery {
    still_owned: bool,
    state: RowState,
}

/// Drive the production terminal write and read the row it leaves.
fn retire_delivery(environment: &CanaryEnvironment, id: JobId<'_>) -> RetiredDelivery {
    let still_owned =
        record_terminal_failure(environment, id.as_str(), CRASHED_WORKER, RETIRED_CAP as u32);
    RetiredDelivery {
        still_owned,
        state: read_state(&environment.runtime, &environment.pool, id.as_str()),
    }
}

/// Backdate the crashed worker and return the row recovery hands back.
fn recover_retired_row(
    environment: &CanaryEnvironment,
    queue: QueueName<'_>,
    id: JobId<'_>,
) -> RowState {
    environment.run(mark_worker_stale(&environment.pool, CRASHED_WORKER));
    recover_orphaned_row(environment, queue, WorkerName(&unique_worker_name()), id);
    read_state(&environment.runtime, &environment.pool, id.as_str())
}

/// Scenario: the terminating delivery recorded its terminal failure and died
/// before its acknowledgement, and orphan recovery returned the row.
///
/// Invariant: the pipeline refuses the recovered delivery without reaching the
/// handler, then acknowledges it into a `Killed` row past the cap. This drives
/// the whole window through production code — the real terminal write, the real
/// recovery stream, and a real worker — rather than seeding the row recovery
/// would have left behind. The terminal write is also shown to preserve the
/// claim token beside the reason it stores.
#[test]
fn the_worker_pipeline_refuses_a_row_recovered_from_a_terminal_write() {
    let environment = CanaryEnvironment::shared();
    let pool = environment.pool.clone();
    let id = apalis_job_id();
    let queue = QueueName(TERMINAL_WINDOW_QUEUE);
    let job = JobId(&id);

    seed_crashed_delivery(environment, queue, job);

    let retired = retire_delivery(environment, job);
    assert!(
        retired.still_owned,
        "the terminating delivery still owns a running row"
    );
    assert_eq!(
        retired.state.status, "Running",
        "the terminal write leaves the status for the acknowledgement"
    );
    assert_eq!(
        retired.state.attempts,
        RETIRED_CAP - 1,
        "the terminal write leaves the attempt count for the acknowledgement"
    );
    assert_eq!(
        retired.state.claim_token.as_deref(),
        Some(CRASHED_CLAIM),
        "storing the terminal reason must preserve the claim token beside it"
    );

    let recovered = recover_retired_row(environment, queue, job);
    assert_eq!(
        recovered.status, "Pending",
        "orphan recovery returns the retired row to the fetcher"
    );
    assert_eq!(
        recovered.attempts, RECOVERED_ATTEMPTS,
        "orphan recovery increments the retired row's attempt count"
    );

    let report = environment
        .run(run_queue_retry_worker(
            worker_config(&pool, TERMINAL_WINDOW_QUEUE, &id),
            succeeding_handler,
        ))
        .expect("the worker run completes within its deadline");

    assert!(
        report.reached_terminal,
        "the refused delivery is acknowledged into a terminal row"
    );
    assert_eq!(
        report.runs_at_terminal, 0,
        "a row recovered after its terminal write must not reach the handler"
    );
    let state = read_state(&environment.runtime, &pool, &id);
    assert_eq!(
        state.status, "Killed",
        "the acknowledgement records the recovered delivery that never ran"
    );
    assert_eq!(
        state.attempts,
        RETIRED_CAP + 1,
        "the acknowledgement stores the attempt the recovered delivery reported"
    );
}

/// Scenario: the first delivery of a row whose cap is one.
///
/// Invariant: the final permitted attempt is legal and runs, so the spent
/// attempt guard refuses only deliveries past the cap. This is the worker
/// pipeline's positive control for the unit case that retains the same
/// boundary, `delivery_at_the_stored_cap_still_reaches_the_handler`.
#[test]
fn the_worker_pipeline_runs_the_final_permitted_attempt() {
    let environment = CanaryEnvironment::shared();
    let pool = environment.pool.clone();
    let id = apalis_job_id();
    let queue = "canary.worker-final-attempt";

    environment.run(async {
        // The fetch delivers one more than the stored count, so a row holding
        // one below the cap is delivered at the cap itself.
        insert_job(
            &pool,
            JobRow::new(queue, &id, "Pending")
                .attempts(RETIRED_CAP - 1)
                .max_attempts(RETIRED_CAP),
        )
        .await;
    });

    let report = environment
        .run(run_queue_retry_worker(
            worker_config(&pool, queue, &id),
            succeeding_handler,
        ))
        .expect("the worker run completes within its deadline");

    assert!(report.reached_terminal, "the successful delivery finishes");
    assert_eq!(
        report.runs_at_terminal, 1,
        "the delivery at the cap must reach the handler exactly once"
    );
    let state = read_state(&environment.runtime, &pool, &id);
    assert_eq!(state.status, "Done", "the successful handler stores Done");
    assert_eq!(
        state.attempts, RETIRED_CAP,
        "the acknowledgement stores the delivered attempt"
    );
}
