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
use apalis_core::task::task_id::TaskId;
use apalis_core::worker::context::WorkerContext;
use apalis_postgres::{PgContext, PgTask};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use sqlx::Error as SqlxError;
use ulid::Ulid;

use super::super::{RetryingJobService, RetryingJobServiceConfig};
use super::policy;
use crate::domain::jobs::retry::JobFailure;
use crate::domain::ports::{
    RouteDeadLetterOutcome, RouteQueueMetrics, RouteQueueOutcome, RouteRetryOutcome,
};
use crate::outbound::queue::{RetryPersistence, TerminalRetryReason};

pub(super) const QUEUE: &str = "wildside.wrapper-effects";
pub(super) const WORKER: &str = "wrapper-effects-worker";
pub(super) const TASK_ID: &str = "01J00000000000000000000000";

/// Concrete `v1` envelope the wrapper must decode before invoking the handler.
#[derive(Clone, Debug, Deserialize)]
pub(super) struct TestJob {
    #[serde(rename = "probeId")]
    #[allow(dead_code)] // Decoding is the behaviour under test.
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
        task_id: &str,
        worker_id: &str,
        attempt: NonZeroU32,
        reason: TerminalRetryReason,
        shrink_limit: bool,
    ) -> Result<bool, SqlxError> {
        self.record(PersistenceCall::Terminal {
            task_id: task_id.to_owned(),
            worker_id: worker_id.to_owned(),
            attempt: attempt.get(),
            reason,
            shrink_limit,
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

pub(super) fn task(payload: Value, attempt: usize) -> PgTask<Value> {
    let mut data = Extensions::default();
    // `WorkerContext::new` carries the service type only for diagnostics, so
    // the unit is sufficient here.
    data.insert(WorkerContext::new::<()>(WORKER));
    Task::builder(payload)
        .with_ctx(
            PgContext::new()
                .with_max_attempts(5)
                .with_lock_by(Some(WORKER.to_owned())),
        )
        .with_data(data)
        .with_task_id(TaskId::new(Ulid::new()))
        .with_attempt(Attempt::new_with_value(attempt))
        .build()
}

pub(super) fn service<F, Fut>(
    persistence: Arc<RecordingPersistence>,
    metrics: Arc<RecordingMetrics>,
    handler: F,
) -> RetryingJobService<TestJob, F>
where
    F: Fn(TestJob, NonZeroU32) -> Fut + Clone + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<(), JobFailure>> + Send + 'static,
{
    let config = RetryingJobServiceConfig::new(
        QUEUE.to_owned(),
        policy(3),
        persistence,
        metrics as Arc<dyn RouteQueueMetrics>,
    );
    RetryingJobService::<TestJob, F>::new(config, handler)
}
