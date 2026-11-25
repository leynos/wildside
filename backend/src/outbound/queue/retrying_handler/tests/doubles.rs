//! Shared doubles for the retry wrapper's unit tests.
//!
//! The wrapper talks to its scheduler through [`RetryPersistence`] and to
//! metrics through [`RouteQueueMetrics`]. These doubles record every call so a
//! test can assert how many writes a delivery produced instead of inferring it
//! from database state. The task builders live in
//! [`deliveries`](super::deliveries) and are re-exported here, so a test
//! imports one fixture module.

use std::collections::VecDeque;
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use sqlx::Error as SqlxError;

use super::super::{JitterSource, RetryingJobService, RetryingJobServiceConfig};
use super::policy;
use crate::domain::jobs::retry::{JitterSample, JobFailure, JobRetryPolicy};
use crate::domain::ports::{
    DeadReason, RouteDeadLetterOutcome, RouteQueueMetrics, RouteQueueOutcome, RouteRetryOutcome,
};
use crate::outbound::queue::{
    RetryPersistence, RetryScheduleUpdate, TerminalCapClamp, TerminalRetryUpdate,
};

pub(super) const QUEUE: &str = "wildside.wrapper-effects";
pub(super) const WORKER: &str = "wrapper-effects-worker";
pub(super) const TASK_ID: &str = "01J00000000000000000000000";

/// Claim token a delivery that passed through the lock step carries.
///
/// The production lock stamps a random token before the wrapper runs, so the
/// claimed-delivery fixtures carry a sentinel and the effect tests assert it
/// reaches every guarded write. A tokenless delivery is covered separately.
pub(super) const CLAIM_TOKEN: &str = "wrapper-effects-claim-token";

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
        claim_token: String,
        attempt: u32,
        delay: Duration,
    },
    Terminal {
        task_id: String,
        worker_id: String,
        claim_token: String,
        attempt: u32,
        reason: DeadReason,
        shrink_limit: bool,
    },
    Clamp {
        task_id: String,
        worker_id: String,
        claim_token: String,
        attempt: u32,
    },
}

/// Scheduler double that records calls and returns a scripted outcome.
pub(super) struct RecordingPersistence {
    schedule: WriteOutcome,
    terminal: WriteOutcome,
    clamps: Mutex<VecDeque<WriteOutcome>>,
    clamp_default: WriteOutcome,
    calls: Mutex<Vec<PersistenceCall>>,
}

impl RecordingPersistence {
    /// Build a double whose fallback cap write succeeds.
    ///
    /// Only the tests that drive a failed terminal write care how the fallback
    /// answers, so it starts out applied and
    /// [`with_clamp_script`](Self::with_clamp_script) changes it.
    pub(super) fn new(schedule: WriteOutcome, terminal: WriteOutcome) -> Self {
        Self {
            schedule,
            terminal,
            clamps: Mutex::new(VecDeque::new()),
            clamp_default: WriteOutcome::Updated,
            calls: Mutex::new(Vec::new()),
        }
    }

    /// Choose how each successive fallback cap write answers.
    ///
    /// The wrapper retries a failed fallback, so a test needs one outcome per
    /// attempt. The last scripted outcome answers any further call, which lets
    /// a test say "it keeps failing" once instead of counting attempts.
    pub(super) fn with_clamp_script(
        mut self,
        outcomes: impl IntoIterator<Item = WriteOutcome>,
    ) -> Self {
        let clamps = outcomes.into_iter().collect::<VecDeque<_>>();
        if let Some(last) = clamps.back() {
            self.clamp_default = *last;
        }
        self.clamps = Mutex::new(clamps);
        self
    }

    pub(super) fn calls(&self) -> Vec<PersistenceCall> {
        self.calls.lock().expect("persistence calls").clone()
    }

    fn record(&self, call: PersistenceCall) {
        self.calls.lock().expect("persistence calls").push(call);
    }

    fn next_clamp_outcome(&self) -> WriteOutcome {
        self.clamps
            .lock()
            .expect("clamp script")
            .pop_front()
            .unwrap_or(self.clamp_default)
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
    async fn schedule_retry(&self, update: RetryScheduleUpdate<'_>) -> Result<bool, SqlxError> {
        self.record(PersistenceCall::Schedule {
            task_id: update.task_id.to_owned(),
            worker_id: update.worker_id.to_owned(),
            claim_token: update.claim_token.to_owned(),
            attempt: update.attempt.get(),
            delay: update.delay,
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
            claim_token: update.claim_token.to_owned(),
            attempt: update.attempt.get(),
            reason: update.reason,
            shrink_limit: update.shrink_limit,
        });
        outcome(self.terminal)
    }

    async fn clamp_terminal_cap(&self, clamp: TerminalCapClamp<'_>) -> Result<bool, SqlxError> {
        self.record(PersistenceCall::Clamp {
            task_id: clamp.task_id.to_owned(),
            worker_id: clamp.worker_id.to_owned(),
            claim_token: clamp.claim_token.to_owned(),
            attempt: clamp.attempt.get(),
        });
        outcome(self.next_clamp_outcome())
    }
}

#[derive(Default)]
pub(super) struct RecordingMetrics {
    retries: Mutex<Vec<RouteRetryOutcome>>,
    dead_letters: Mutex<Vec<RouteDeadLetterOutcome>>,
    terminal_write_failures: Mutex<Vec<RouteDeadLetterOutcome>>,
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

    fn observe_terminal_write_failure(&self, reason: RouteDeadLetterOutcome) {
        self.terminal_write_failures
            .lock()
            .expect("terminal-write-failure metrics")
            .push(reason);
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

    pub(super) fn terminal_write_failures(&self) -> Vec<RouteDeadLetterOutcome> {
        self.terminal_write_failures
            .lock()
            .expect("terminal-write-failure metrics")
            .clone()
    }
}

pub(super) use super::deliveries::{DeliveryIdentity, task, task_with, task_with_budget};

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
