//! PostgreSQL canaries for the retry wrapper's ownership precondition.
//!
//! The wrapper may only persist a failure outcome while it can prove the
//! current worker owns the row. These canaries drive the real service against
//! a real `apalis.jobs` row, so they prove the refusal at the storage boundary
//! rather than only through recording doubles.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use apalis_core::error::BoxDynError;
use apalis_core::layers::Service;
use apalis_core::task::Task;
use apalis_core::task::attempt::Attempt;
use apalis_core::task::extensions::Extensions;
use apalis_core::worker::context::WorkerContext;
use apalis_postgres::{PgContext, PgTask};
use backend::domain::jobs::retry::{
    JobErrorKind, JobFailure, JobRetryPolicy, JobRetryPolicyConfig,
};
use backend::domain::ports::NoOpRouteQueueMetrics;
use backend::outbound::queue::{
    PostgresRetryScheduler, RetryPersistence, RetryingJobService, RetryingJobServiceConfig,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::{CanaryEnvironment, JobRow, insert_job, register_worker};
use crate::support::apalis_job_id::apalis_job_id;

/// Typed `v1` envelope the production wrapper decodes before invoking the
/// handler.
#[derive(Clone, Debug, Deserialize)]
struct ProbeJob {
    #[serde(rename = "probeId")]
    #[expect(dead_code, reason = "decoding is the behaviour under test")]
    probe_id: String,
}

/// Scheduling state the wrapper must leave untouched when it refuses to write.
#[derive(Debug, PartialEq, Eq)]
struct SchedulingState {
    status: String,
    attempts: i32,
    max_attempts: i32,
    lock_by: Option<String>,
    run_at_micros: i64,
    metadata: Option<String>,
}

fn read_scheduling_state(
    runtime: &tokio::runtime::Runtime,
    pool: &PgPool,
    id: &str,
) -> SchedulingState {
    runtime.block_on(async {
        let row: (String, i32, i32, Option<String>, i64, Option<String>) = sqlx::query_as(
            "SELECT status, attempts, max_attempts, lock_by,
                    (extract(epoch FROM run_at) * 1000000)::bigint,
                    metadata::text
             FROM apalis.jobs WHERE id = $1",
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("read canary job state");
        SchedulingState {
            status: row.0,
            attempts: row.1,
            max_attempts: row.2,
            lock_by: row.3,
            run_at_micros: row.4,
            metadata: row.5,
        }
    })
}

/// Build a delivery whose worker context and `lock_by` are chosen separately.
fn task_with_identity(
    id: &str,
    worker: &str,
    lock_by: Option<&str>,
    attempt: usize,
) -> PgTask<Value> {
    let mut data = Extensions::default();
    data.insert(WorkerContext::new::<Value>(worker));
    Task::builder(json!({ "v": "v1", "probeId": id }))
        .with_ctx(
            PgContext::new()
                .with_max_attempts(5)
                .with_lock_by(lock_by.map(str::to_owned)),
        )
        .with_data(data)
        .with_task_id(id.parse().expect("canary task id"))
        .with_attempt(Attempt::new_with_value(attempt))
        .build()
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

/// Deliver one retryable failure through the production wrapper.
async fn deliver(pool: &PgPool, queue: &str, task: PgTask<Value>) -> Result<(), BoxDynError> {
    let scheduler: Arc<dyn RetryPersistence> = Arc::new(PostgresRetryScheduler::new(pool.clone()));
    let config = RetryingJobServiceConfig::new(
        queue.to_owned(),
        policy(),
        scheduler,
        Arc::new(NoOpRouteQueueMetrics),
    );
    let handler = |_job: ProbeJob, _attempt| async {
        Err(JobFailure::new(JobErrorKind::Retryable, "probe failure"))
    };
    let mut service = RetryingJobService::<ProbeJob, _>::new(config, handler);
    service.call(task).await
}

/// Insert a running row the canary worker owns.
async fn insert_running_row(pool: &PgPool, queue: &str, worker: &str, id: &str) {
    register_worker(pool, worker).await;
    insert_job(
        pool,
        JobRow::new(queue, id, "Running")
            .attempts(1)
            .max_attempts(5)
            .run_at_offset_seconds(3_600.0)
            .metadata(json!({ "keep": "marker" }))
            .locked_by(worker),
    )
    .await;
}

#[test]
fn wrapper_writes_nothing_when_lock_by_is_absent() {
    let environment = CanaryEnvironment::shared();
    let pool = environment.pool.clone();
    let queue = "canary.identity-no-lock";
    let worker = "canary-identity-no-lock-worker";
    let id = apalis_job_id();

    environment.run(insert_running_row(&pool, queue, worker, &id));
    let before = read_scheduling_state(&environment.runtime, &pool, &id);

    let result = environment.run(deliver(
        &pool,
        queue,
        task_with_identity(&id, worker, None, 1),
    ));

    assert!(result.is_err(), "the delivery stays failed");
    assert_eq!(
        read_scheduling_state(&environment.runtime, &pool, &id),
        before,
        "the wrapper must not write without a recorded lock owner"
    );
}

#[test]
fn wrapper_writes_nothing_when_the_lock_owner_disagrees() {
    let environment = CanaryEnvironment::shared();
    let pool = environment.pool.clone();
    let queue = "canary.identity-mismatch";
    let worker = "canary-identity-mismatch-worker";
    let id = apalis_job_id();

    environment.run(insert_running_row(&pool, queue, worker, &id));
    let before = read_scheduling_state(&environment.runtime, &pool, &id);

    let result = environment.run(deliver(
        &pool,
        queue,
        task_with_identity(&id, worker, Some("another-worker"), 1),
    ));

    assert!(result.is_err(), "the delivery stays failed");
    assert_eq!(
        read_scheduling_state(&environment.runtime, &pool, &id),
        before,
        "the wrapper must not write for a worker that does not own the row"
    );
}

#[test]
fn wrapper_schedules_the_retry_when_the_lock_owner_agrees() {
    let environment = CanaryEnvironment::shared();
    let pool = environment.pool.clone();
    let queue = "canary.identity-owned";
    let worker = "canary-identity-owned-worker";
    let id = apalis_job_id();

    environment.run(insert_running_row(&pool, queue, worker, &id));
    let before = read_scheduling_state(&environment.runtime, &pool, &id);

    let result = environment.run(deliver(
        &pool,
        queue,
        task_with_identity(&id, worker, Some(worker), 1),
    ));

    assert!(result.is_err(), "the delivery stays failed");
    let after = read_scheduling_state(&environment.runtime, &pool, &id);
    assert_eq!(after.status, before.status, "the wrapper leaves the status");
    assert_eq!(after.attempts, before.attempts);
    assert_eq!(after.max_attempts, before.max_attempts);
    assert_eq!(after.lock_by, before.lock_by);
    assert_eq!(after.metadata, before.metadata);
    assert_ne!(
        after.run_at_micros, before.run_at_micros,
        "an owned row must receive the scheduled retry deadline"
    );
}
