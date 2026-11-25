//! PostgreSQL canaries for the retry wrapper's ownership precondition.
//!
//! The wrapper may only persist a failure outcome while it can prove the
//! current worker owns the row. These canaries drive the real service against
//! a real `apalis.jobs` row, so they prove the refusal at the storage boundary
//! and assert the metrics the wrapper records. The positive canary claims the
//! row through the production lock step first, so the token its write binds is
//! the one the delivery was actually locked under.

use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
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
use backend::domain::ports::{
    RouteDeadLetterOutcome, RouteQueueMetrics, RouteQueueOutcome, RouteRetryOutcome,
};
use backend::outbound::queue::{
    PostgresRetryScheduler, RetryPersistence, RetryingJobService, RetryingJobServiceConfig,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::fixtures::{
    CanaryEnvironment, GuardedRowState, JobId, JobRow, QueueName, insert_job, read_guarded_row,
    register_worker,
};
use crate::ack_guard::claim_row;
use crate::support::apalis_job_id::apalis_job_id;

/// Typed `v1` envelope the production wrapper decodes before invoking the
/// handler.
#[derive(Clone, Debug, Deserialize)]
struct ProbeJob {
    #[serde(rename = "probeId")]
    #[expect(dead_code, reason = "decoding is the behaviour under test")]
    probe_id: String,
}

/// Metrics double that records retry outcomes.
///
/// The canaries use it to prove the wrapper records the failure metric when it
/// refuses to write, which a no-op recorder could never show.
#[derive(Debug, Default)]
struct RecordingMetrics {
    retries: Mutex<Vec<RouteRetryOutcome>>,
}

impl RecordingMetrics {
    fn retry_outcomes(&self) -> Vec<RouteRetryOutcome> {
        self.retries
            .lock()
            .expect("recording metrics lock is not poisoned")
            .clone()
    }
}

impl RouteQueueMetrics for RecordingMetrics {
    fn observe_enqueue(&self, _outcome: RouteQueueOutcome, _latency: Duration) {}

    fn observe_retry(&self, outcome: RouteRetryOutcome) {
        self.retries
            .lock()
            .expect("recording metrics lock is not poisoned")
            .push(outcome);
    }

    fn observe_dead_letter(&self, _outcome: RouteDeadLetterOutcome) {}

    fn observe_terminal_write_failure(&self, _reason: RouteDeadLetterOutcome) {}
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

/// Identity facts one canary delivery carries.
#[derive(Clone, Copy, Debug)]
enum DeliveryIdentity {
    /// Both the task id and the worker context are present.
    Complete,
    /// The delivery carries no task id.
    MissingTaskId,
    /// The delivery carries no worker context.
    MissingWorkerContext,
}

/// Build a delivery whose worker context, `lock_by`, and identity vary.
///
/// A claimed canary carries the token the production lock step stamped; a
/// synthetic delivery for a refusal case carries none, because the wrapper
/// refuses it before any guarded write.
fn task_for(
    canary: &IdentityCanary,
    lock_by: Option<&str>,
    identity: DeliveryIdentity,
) -> PgTask<Value> {
    let mut data = Extensions::default();
    if !matches!(identity, DeliveryIdentity::MissingWorkerContext) {
        data.insert(WorkerContext::new::<Value>(canary.spec.worker));
    }
    let mut meta = serde_json::Map::new();
    if let Some(token) = canary.claim_token.as_deref() {
        meta.insert(
            "wildside.retry.claim_token".to_owned(),
            Value::String(token.to_owned()),
        );
    }
    let builder = Task::builder(json!({ "v": "v1", "probeId": canary.id }))
        .with_ctx(
            PgContext::new()
                .with_max_attempts(5)
                .with_lock_by(lock_by.map(str::to_owned))
                .with_meta(meta),
        )
        .with_data(data)
        .with_attempt(Attempt::new_with_value(1));
    let builder = match identity {
        DeliveryIdentity::MissingTaskId => builder,
        DeliveryIdentity::Complete | DeliveryIdentity::MissingWorkerContext => {
            builder.with_task_id(canary.id.parse().expect("canary task id"))
        }
    };
    builder.build()
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
async fn deliver(
    pool: &PgPool,
    queue: &str,
    task: PgTask<Value>,
    metrics: Arc<dyn RouteQueueMetrics>,
) -> Result<(), BoxDynError> {
    let scheduler: Arc<dyn RetryPersistence> = Arc::new(PostgresRetryScheduler::new(pool.clone()));
    let config = RetryingJobServiceConfig::new(queue.to_owned(), policy(), scheduler, metrics);
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
    metrics: Arc<RecordingMetrics>,
    /// The token the production lock claim stamped, for a claimed row.
    claim_token: Option<String>,
}

impl IdentityCanary {
    /// Insert a running row the canary worker owns.
    ///
    /// This row supports the refusal canaries, which drive a delivery whose
    /// identity the lock step could never produce and never write.
    fn start(spec: &'static CanarySpec) -> Self {
        let canary = Self {
            environment: CanaryEnvironment::shared(),
            spec,
            id: apalis_job_id(),
            metrics: Arc::new(RecordingMetrics::default()),
            claim_token: None,
        };
        canary.environment.run(insert_running_row(&canary));
        canary
    }

    /// Insert a due row and claim it through the production lock step.
    ///
    /// The claim stamps the row and the delivery context with the same token,
    /// so this is the state the wrapper's guarded writes require. The wrapper's
    /// own write then proves the token reached it.
    fn start_claimed(spec: &'static CanarySpec) -> Self {
        let environment = CanaryEnvironment::shared();
        let id = apalis_job_id();
        environment.run(insert_pending_row(environment, spec, &id));
        let token = claim_row(environment, spec.worker, QueueName(spec.queue), JobId(&id));
        Self {
            environment,
            spec,
            id,
            metrics: Arc::new(RecordingMetrics::default()),
            claim_token: Some(token),
        }
    }

    fn state(&self) -> GuardedRowState {
        read_guarded_row(&self.environment.runtime, &self.environment.pool, &self.id)
    }

    fn retry_outcomes(&self) -> Vec<RouteRetryOutcome> {
        self.metrics.retry_outcomes()
    }

    /// Deliver one retryable failure under the chosen lock owner.
    fn deliver(&self, owner: LockOwner) -> Result<(), BoxDynError> {
        self.deliver_with_identity(owner, DeliveryIdentity::Complete)
    }

    /// Deliver one retryable failure with the chosen lock owner and identity.
    fn deliver_with_identity(
        &self,
        owner: LockOwner,
        identity: DeliveryIdentity,
    ) -> Result<(), BoxDynError> {
        // The annotation is the coercion site: `Arc::clone` would infer its
        // own parameter as the trait object and reject the concrete argument.
        let metrics: Arc<dyn RouteQueueMetrics> = self.metrics.clone();
        self.environment.run(deliver(
            &self.environment.pool,
            self.spec.queue,
            task_for(self, owner.lock_by(self.spec), identity),
            metrics,
        ))
    }
}

/// Insert a running row this worker holds for its first delivery.
///
/// Apalis increments the in-memory attempt on the first poll, so the delivery
/// below counts 1 while the row still records 0. The scheduler's delivery guard
/// matches that recorded value, which is why the fixture seeds 0 rather than 1.
async fn insert_running_row(canary: &IdentityCanary) {
    register_worker(&canary.environment.pool, canary.spec.worker).await;
    insert_job(
        &canary.environment.pool,
        JobRow::new(canary.spec.queue, &canary.id, "Running")
            .attempts(0)
            .max_attempts(5)
            .run_at_offset_seconds(3_600.0)
            .metadata(json!({ "keep": "marker" }))
            .locked_by(canary.spec.worker),
    )
    .await;
}

/// Insert a due `Pending` row the production fetch can hand to the lock step.
async fn insert_pending_row(environment: &CanaryEnvironment, spec: &CanarySpec, id: &str) {
    register_worker(&environment.pool, spec.worker).await;
    insert_job(
        &environment.pool,
        JobRow::new(spec.queue, id, "Pending")
            .attempts(0)
            .max_attempts(5)
            .run_at_offset_seconds(-1.0)
            .metadata(json!({ "keep": "marker" })),
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

/// Assert a refused delivery leaves the row untouched and counts one
/// scheduling failure.
///
/// Both refusal routes end in the same observable state, so the invariant is
/// stated once and both canaries stay in step.
fn assert_refused_without_writing(
    canary: &IdentityCanary,
    deliver: impl FnOnce(&IdentityCanary) -> Result<(), BoxDynError>,
    reason: &str,
) {
    let before = canary.state();

    let result = deliver(canary);

    assert!(result.is_err(), "the delivery stays failed");
    assert_eq!(canary.state(), before, "{reason}");
    assert_eq!(
        canary.retry_outcomes(),
        vec![RouteRetryOutcome::ScheduleFailure],
        "a refused retry counts as a scheduling failure"
    );
}

#[rstest::rstest]
#[case::absent_lock_owner(LockOwner::Missing)]
#[case::disagreeing_lock_owner(LockOwner::OtherWorker)]
#[test]
fn wrapper_writes_nothing_without_a_matching_lock_owner(#[case] owner: LockOwner) {
    let canary = IdentityCanary::start(&REFUSED);

    assert_refused_without_writing(
        &canary,
        |canary| canary.deliver(owner),
        "the wrapper must not write without a lock owner that matches the worker context",
    );
}

#[rstest::rstest]
#[case::absent_task_id(DeliveryIdentity::MissingTaskId)]
#[case::absent_worker_context(DeliveryIdentity::MissingWorkerContext)]
#[test]
fn wrapper_writes_nothing_without_proven_identity(#[case] identity: DeliveryIdentity) {
    let canary = IdentityCanary::start(&REFUSED);

    assert_refused_without_writing(
        &canary,
        |canary| canary.deliver_with_identity(LockOwner::Worker, identity),
        "the wrapper must not write without a task id and a matching worker context",
    );
}

#[test]
fn wrapper_schedules_the_retry_when_the_lock_owner_agrees() {
    let canary = IdentityCanary::start_claimed(&OWNED);
    let before = canary.state();

    let result = canary.deliver(LockOwner::Worker);

    assert!(result.is_err(), "the delivery stays failed");
    let after = canary.state();
    assert_eq!(after.status, before.status, "the wrapper leaves the status");
    assert_eq!(after.attempts, before.attempts);
    assert_eq!(after.max_attempts, before.max_attempts);
    assert_eq!(after.lock_by, before.lock_by);
    assert_eq!(after.metadata, before.metadata);
    assert_eq!(
        after.claim_token, before.claim_token,
        "an owned write must leave the claimed token in place"
    );
    assert_ne!(
        after.run_at_micros, before.run_at_micros,
        "an owned row must receive the scheduled retry deadline"
    );
    assert_eq!(
        canary.retry_outcomes(),
        vec![RouteRetryOutcome::Scheduled],
        "an applied retry update records the scheduled outcome"
    );
}
