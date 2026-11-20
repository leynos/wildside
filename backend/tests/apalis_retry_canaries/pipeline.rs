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
use sqlx::PgPool;

use super::fixtures::{CanaryEnvironment, JobRow, insert_job, read_state};
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
