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

/// The row and worker names one canary scenario uses.
struct CanarySpec {
    queue: &'static str,
    worker: &'static str,
}

/// The `lock_by` a delivery carries, relative to the canary worker.
#[derive(Clone, Copy, Debug)]
enum LockOwner {
    /// The row records no owner at all.
    Missing,
    /// The row is owned by a worker other than the delivery's.
    OtherWorker,
    /// The row is owned by the delivery's own worker.
    Worker,
}

const OTHER_WORKER: &str = "another-worker";

impl LockOwner {
    fn lock_by(self, spec: &CanarySpec) -> Option<&'static str> {
        match self {
            Self::Missing => None,
            Self::OtherWorker => Some(OTHER_WORKER),
            Self::Worker => Some(spec.worker),
        }
    }
}

/// Build a delivery whose worker context and `lock_by` are chosen separately.
fn task_for(spec: &CanarySpec, id: &str, lock_by: Option<&str>) -> PgTask<Value> {
    let mut data = Extensions::default();
    data.insert(WorkerContext::new::<Value>(spec.worker));
    Task::builder(json!({ "v": "v1", "probeId": id }))
        .with_ctx(
            PgContext::new()
                .with_max_attempts(5)
                .with_lock_by(lock_by.map(str::to_owned)),
        )
        .with_data(data)
        .with_task_id(id.parse().expect("canary task id"))
        .with_attempt(Attempt::new_with_value(1))
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

/// One shared canary row, the names its scenario uses, and its environment.
struct IdentityCanary {
    environment: &'static CanaryEnvironment,
    spec: &'static CanarySpec,
    id: String,
}

impl IdentityCanary {
    /// Insert a running row the canary worker owns.
    fn start(spec: &'static CanarySpec) -> Self {
        let canary = Self {
            environment: CanaryEnvironment::shared(),
            spec,
            id: apalis_job_id(),
        };
        canary.environment.run(insert_running_row(&canary));
        canary
    }

    fn state(&self) -> SchedulingState {
        read_scheduling_state(&self.environment.runtime, &self.environment.pool, &self.id)
    }

    /// Deliver one retryable failure under the chosen lock owner.
    fn deliver(&self, owner: LockOwner) -> Result<(), BoxDynError> {
        self.environment.run(deliver(
            &self.environment.pool,
            self.spec.queue,
            task_for(self.spec, &self.id, owner.lock_by(self.spec)),
        ))
    }
}

async fn insert_running_row(canary: &IdentityCanary) {
    register_worker(&canary.environment.pool, canary.spec.worker).await;
    insert_job(
        &canary.environment.pool,
        JobRow::new(canary.spec.queue, &canary.id, "Running")
            .attempts(1)
            .max_attempts(5)
            .run_at_offset_seconds(3_600.0)
            .metadata(json!({ "keep": "marker" }))
            .locked_by(canary.spec.worker),
    )
    .await;
}

const REFUSED: CanarySpec = CanarySpec {
    queue: "canary.identity-refused",
    worker: "canary-identity-refused-worker",
};

const OWNED: CanarySpec = CanarySpec {
    queue: "canary.identity-owned",
    worker: "canary-identity-owned-worker",
};

#[rstest::rstest]
#[case::absent_lock_owner(LockOwner::Missing)]
#[case::disagreeing_lock_owner(LockOwner::OtherWorker)]
#[test]
fn wrapper_writes_nothing_without_a_matching_lock_owner(#[case] owner: LockOwner) {
    let canary = IdentityCanary::start(&REFUSED);
    let before = canary.state();

    let result = canary.deliver(owner);

    assert!(result.is_err(), "the delivery stays failed");
    assert_eq!(
        canary.state(),
        before,
        "the wrapper must not write without a lock owner that matches the worker context"
    );
}

#[test]
fn wrapper_schedules_the_retry_when_the_lock_owner_agrees() {
    let canary = IdentityCanary::start(&OWNED);
    let before = canary.state();

    let result = canary.deliver(LockOwner::Worker);

    assert!(result.is_err(), "the delivery stays failed");
    let after = canary.state();
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
