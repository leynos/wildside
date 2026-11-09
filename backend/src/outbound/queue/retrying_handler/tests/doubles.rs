//! Shared doubles for the retry wrapper's unit tests.
//!
//! The wrapper talks to its scheduler through [`RetryPersistence`] and to
//! metrics through [`RouteQueueMetrics`]. These doubles record every call so a
//! test can assert how many writes a delivery produced instead of inferring it
//! from database state.

use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use apalis_core::task::Task;
use apalis_core::task::attempt::Attempt;
use apalis_core::task::extensions::Extensions;
use apalis_core::worker::context::WorkerContext;
use apalis_postgres::{PgContext, PgTask};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use sqlx::Error as SqlxError;

use super::super::{JitterSource, RetryingJobService, RetryingJobServiceConfig};
use super::policy;
use crate::domain::jobs::retry::{JitterSample, JobFailure, JobRetryPolicy};
use crate::domain::ports::{
    RouteDeadLetterOutcome, RouteQueueMetrics, RouteQueueOutcome, RouteRetryOutcome,
};
use crate::outbound::queue::{RetryPersistence, TerminalRetryReason, TerminalRetryUpdate};

pub(super) const QUEUE: &str = "wildside.wrapper-effects";
pub(super) const WORKER: &str = "wrapper-effects-worker";
pub(super) const TASK_ID: &str = "01J00000000000000000000000";

/// Concrete `v1` envelope the wrapper must decode before invoking the handler.
#[derive(Clone, Debug, Deserialize)]
pub(super) struct TestJob {
    #[serde(rename = "probeId")]
    #[expect(dead_code, reason = "decoding is the behaviour under test")]
    probe_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WriteOutcome {
    Updated,
    NotOwned,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum PersistenceCall {
    Schedule {
        task_id: String,
        worker_id: String,
        delay: Duration,
    },
    Terminal {
        task_id: String,
        worker_id: String,
        attempt: u32,
        reason: TerminalRetryReason,
        shrink_limit: bool,
    },
}

/// Scheduler double that records calls and returns a fixed outcome.
pub(super) struct RecordingPersistence {
    schedule: WriteOutcome,
    terminal: WriteOutcome,
    calls: Mutex<Vec<PersistenceCall>>,
}

impl RecordingPersistence {
    pub(super) fn new(schedule: WriteOutcome, terminal: WriteOutcome) -> Self {
        Self {
            schedule,
            terminal,
            calls: Mutex::new(Vec::new()),
        }
    }

    pub(super) fn calls(&self) -> Vec<PersistenceCall> {
        self.calls.lock().expect("persistence calls").clone()
    }

    fn record(&self, call: PersistenceCall) {
        self.calls.lock().expect("persistence calls").push(call);
    }
}

fn outcome(outcome: WriteOutcome) -> Result<bool, SqlxError> {
    match outcome {
        WriteOutcome::Updated => Ok(true),
        WriteOutcome::NotOwned => Ok(false),
        WriteOutcome::Failed => Err(SqlxError::Protocol("simulated write failure".to_owned())),
    }
}

#[async_trait]
impl RetryPersistence for RecordingPersistence {
    async fn schedule_retry(
        &self,
        task_id: &str,
        worker_id: &str,
        delay: Duration,
    ) -> Result<bool, SqlxError> {
        self.record(PersistenceCall::Schedule {
            task_id: task_id.to_owned(),
            worker_id: worker_id.to_owned(),
            delay,
        });
        outcome(self.schedule)
    }

    async fn record_terminal_failure(
        &self,
        update: TerminalRetryUpdate<'_>,
    ) -> Result<bool, SqlxError> {
        self.record(PersistenceCall::Terminal {
            task_id: update.task_id.to_owned(),
            worker_id: update.worker_id.to_owned(),
            attempt: update.attempt.get(),
            reason: update.reason,
            shrink_limit: update.shrink_limit,
        });
        outcome(self.terminal)
    }
}

#[derive(Default)]
pub(super) struct RecordingMetrics {
    retries: Mutex<Vec<RouteRetryOutcome>>,
    dead_letters: Mutex<Vec<RouteDeadLetterOutcome>>,
}

impl RouteQueueMetrics for RecordingMetrics {
    fn observe_enqueue(&self, _outcome: RouteQueueOutcome, _latency: Duration) {}

    fn observe_retry(&self, outcome: RouteRetryOutcome) {
        self.retries.lock().expect("retry metrics").push(outcome);
    }

    fn observe_dead_letter(&self, outcome: RouteDeadLetterOutcome) {
        self.dead_letters
            .lock()
            .expect("dead-letter metrics")
            .push(outcome);
    }
}

impl RecordingMetrics {
    pub(super) fn retries(&self) -> Vec<RouteRetryOutcome> {
        self.retries.lock().expect("retry metrics").clone()
    }

    pub(super) fn dead_letters(&self) -> Vec<RouteDeadLetterOutcome> {
        self.dead_letters
            .lock()
            .expect("dead-letter metrics")
            .clone()
    }
}

/// Build a delivery whose worker context and `lock_by` agree.
pub(super) fn task(payload: Value, attempt: usize) -> PgTask<Value> {
    task_with(
        payload,
        attempt,
        DeliveryIdentity::new(Some(WORKER), Some(WORKER)),
    )
}

/// The identity a task double carries.
///
/// The wrapper writes only when it can name both the row and the worker that
/// owns it, so tests vary these three fields independently. The identity
/// starts out naming [`TASK_ID`]; [`without_task_id`](Self::without_task_id)
/// removes that name to exercise the refusal path.
#[derive(Clone, Copy, Debug)]
pub(super) struct DeliveryIdentity<'a> {
    worker: Option<&'a str>,
    lock_by: Option<&'a str>,
    task_id: Option<&'a str>,
}

impl<'a> DeliveryIdentity<'a> {
    /// An identity for a delivery that names [`TASK_ID`].
    ///
    /// `worker` is the name stored in the task's [`WorkerContext`]; `lock_by`
    /// is the worker recorded on the Apalis context. The wrapper persists a
    /// failure only when both are present and agree, so tests pass each
    /// missing or mismatched combination.
    pub(super) fn new(worker: Option<&'a str>, lock_by: Option<&'a str>) -> Self {
        Self {
            worker,
            lock_by,
            task_id: Some(TASK_ID),
        }
    }

    /// The same identity on a delivery that carries no task identifier.
    ///
    /// The wrapper needs the identifier to address the row it would update, so
    /// a delivery without one must be refused even when the worker identities
    /// agree.
    pub(super) fn without_task_id(mut self) -> Self {
        self.task_id = None;
        self
    }
}

/// Build a delivery with the chosen identity.
pub(super) fn task_with(
    payload: Value,
    attempt: usize,
    identity: DeliveryIdentity<'_>,
) -> PgTask<Value> {
    let mut data = Extensions::default();
    if let Some(worker) = identity.worker {
        // `WorkerContext::new` carries the service type only for diagnostics,
        // so the unit is sufficient here.
        data.insert(WorkerContext::new::<()>(worker));
    }
    let mut builder = Task::builder(payload)
        .with_ctx(
            PgContext::new()
                .with_max_attempts(5)
                .with_lock_by(identity.lock_by.map(str::to_owned)),
        )
        .with_data(data)
        .with_attempt(Attempt::new_with_value(attempt));
    if let Some(task_id) = identity.task_id {
        builder = builder.with_task_id(task_id.parse().expect("task id"));
    }
    builder.build()
}

/// Jitter source that always returns the same sample.
pub(super) struct FixedJitter(pub(super) JitterSample);

impl JitterSource for FixedJitter {
    fn sample(&self) -> JitterSample {
        self.0
    }
}

/// Build one wrapper configuration for the shared queue under test.
///
/// Callers that care about jitter use this to reach
/// `RetryingJobServiceConfig::with_jitter` themselves, so the wiring they
/// exercise is the same one production uses.
pub(super) fn wrapper_config(
    policy: JobRetryPolicy,
    persistence: Arc<RecordingPersistence>,
    metrics: Arc<RecordingMetrics>,
) -> RetryingJobServiceConfig {
    RetryingJobServiceConfig::new(
        QUEUE.to_owned(),
        policy,
        persistence,
        metrics as Arc<dyn RouteQueueMetrics>,
    )
}

/// Build a service with the shared test policy and a midpoint jitter sample.
///
/// The midpoint sample leaves every nominal delay unchanged, so tests that
/// only care about which write a delivery produces are unaffected by jitter.
pub(super) fn service<F, Fut>(
    persistence: Arc<RecordingPersistence>,
    metrics: Arc<RecordingMetrics>,
    handler: F,
) -> RetryingJobService<TestJob, F>
where
    F: Fn(TestJob, NonZeroU32) -> Fut + Clone + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<(), JobFailure>> + Send + 'static,
{
    let config = wrapper_config(policy(3), persistence, metrics)
        .with_jitter(Arc::new(FixedJitter(JitterSample::midpoint())));
    RetryingJobService::<TestJob, F>::new(config, handler)
}

/// Build a service from a prepared configuration.
///
/// Tests that vary a configuration input, such as the jitter source, build the
/// configuration first and then hand it over unchanged.
pub(super) fn service_with<F, Fut>(
    config: RetryingJobServiceConfig,
    handler: F,
) -> RetryingJobService<TestJob, F>
where
    F: Fn(TestJob, NonZeroU32) -> Fut + Clone + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<(), JobFailure>> + Send + 'static,
{
    RetryingJobService::<TestJob, F>::new(config, handler)
}
