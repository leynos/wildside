//! Shared harness that drives one failed delivery through the wrapper.
//!
//! The effect modules assert which writes a single failed delivery produces.
//! They all need the same fixed attempt, the same recording doubles, and the
//! same expectation that the wrapper returns an error, so those live here
//! rather than being restated in each module.

use std::num::NonZeroU32;
use std::time::Duration;

use super::super::{RetryFailureContext, handle_failure};
use super::doubles::{
    CLAIM_TOKEN, FixedJitter, PersistenceCall, QUEUE, RecordingMetrics, RecordingPersistence,
    TASK_ID, WORKER, WriteOutcome,
};
use super::policy;
use crate::domain::jobs::retry::{JitterSample, JobErrorKind, JobFailure};
use crate::domain::ports::{DeadReason, RouteDeadLetterOutcome, RouteRetryOutcome};

/// Delivery attempt every effect test exercises.
///
/// The wrapper's decision table is covered elsewhere; these tests only care
/// which write a single failed delivery produces, so the attempt and the
/// task's own attempt limit are held fixed.
pub(super) const ATTEMPT: u32 = 1;
pub(super) const CONTEXT_MAX_ATTEMPTS: u32 = 5;

/// One failed delivery, observed through recording doubles.
pub(super) struct FailureRun {
    pub(super) persistence: RecordingPersistence,
    pub(super) metrics: RecordingMetrics,
}

impl FailureRun {
    pub(super) fn new(schedule: WriteOutcome, terminal: WriteOutcome) -> Self {
        Self {
            persistence: RecordingPersistence::new(schedule, terminal),
            metrics: RecordingMetrics::default(),
        }
    }

    /// A run whose terminal write fails and whose fallback writes follow `clamps`.
    ///
    /// The wrapper retries a failed fallback, so each scripted outcome answers
    /// one attempt.
    pub(super) fn failing_terminal_write(clamps: impl IntoIterator<Item = WriteOutcome>) -> Self {
        Self {
            persistence: RecordingPersistence::new(WriteOutcome::Updated, WriteOutcome::Failed)
                .with_clamp_script(clamps),
            metrics: RecordingMetrics::default(),
        }
    }

    /// Drive one failed delivery through the wrapper's failure path.
    ///
    /// The delivery is the claimed one the lock step hands over, so every
    /// guarded write it causes must carry [`CLAIM_TOKEN`]. The wrapper always
    /// asks Apalis to keep the task failed, so the error is expected and
    /// deliberately dropped here.
    pub(super) async fn apply(&self, kind: JobErrorKind) {
        self.apply_with_identity(kind, Some(TASK_ID), Some(WORKER))
            .await;
    }

    /// Drive one failed delivery with a chosen task and worker identity.
    ///
    /// The scheduler is only reachable when both identities are present; a
    /// `None` on either side exercises the wrapper's refusal to persist. The
    /// delivery carries [`CLAIM_TOKEN`], which the guarded writes must forward.
    pub(super) async fn apply_with_identity(
        &self,
        kind: JobErrorKind,
        task_id: Option<&'static str>,
        worker_id: Option<&'static str>,
    ) {
        self.run_with(kind, (task_id, worker_id), Some(CLAIM_TOKEN))
            .await;
    }

    /// Drive one failed delivery that never passed through the lock step.
    ///
    /// Its guarded writes bind the empty string, which matches no row.
    pub(super) async fn apply_without_claim_token(&self, kind: JobErrorKind) {
        self.run_with(kind, (Some(TASK_ID), Some(WORKER)), None)
            .await;
    }

    /// Drive one failed delivery carrying the chosen identity and claim token.
    async fn run_with(
        &self,
        kind: JobErrorKind,
        identity: (Option<&'static str>, Option<&'static str>),
        claim_token: Option<&'static str>,
    ) {
        let (task_id, worker_id) = identity;
        let retry_policy = policy(3);
        let jitter = FixedJitter(JitterSample::midpoint());
        let context = RetryFailureContext {
            attempt: NonZeroU32::new(ATTEMPT).expect("positive attempt"),
            context_max_attempts: CONTEXT_MAX_ATTEMPTS,
            policy: &retry_policy,
            scheduler: &self.persistence,
            metrics: &self.metrics,
            jitter: &jitter,
            queue_name: QUEUE,
            task_id,
            worker_id,
            claim_token,
        };
        let failure = JobFailure::new(kind, "probe failure");
        handle_failure(failure, &context)
            .await
            .expect_err("the wrapper asks Apalis to keep the task failed");
    }

    pub(super) fn calls(&self) -> Vec<PersistenceCall> {
        self.persistence.calls()
    }

    pub(super) fn retries(&self) -> Vec<RouteRetryOutcome> {
        self.metrics.retries()
    }

    pub(super) fn dead_letters(&self) -> Vec<RouteDeadLetterOutcome> {
        self.metrics.dead_letters()
    }

    pub(super) fn terminal_write_failures(&self) -> Vec<RouteDeadLetterOutcome> {
        self.metrics.terminal_write_failures()
    }
}

/// The terminal write a fatal failure of the claimed delivery performs.
///
/// The token is the one the shared delivery carries, so a wrapper that dropped
/// or replaced it would fail the recorded call.
pub(super) fn terminal_call() -> PersistenceCall {
    PersistenceCall::Terminal {
        task_id: TASK_ID.to_owned(),
        worker_id: WORKER.to_owned(),
        claim_token: CLAIM_TOKEN.to_owned(),
        attempt: ATTEMPT,
        reason: DeadReason::Fatal,
        shrink_limit: true,
    }
}

/// The cap-lowering fallback the claimed delivery performs.
///
/// The wrapper can repeat this write, so every recorded clamp must carry the
/// same claim token as the terminal write it follows.
pub(super) fn clamp_call() -> PersistenceCall {
    PersistenceCall::Clamp {
        task_id: TASK_ID.to_owned(),
        worker_id: WORKER.to_owned(),
        claim_token: CLAIM_TOKEN.to_owned(),
        attempt: ATTEMPT,
    }
}

/// The retry write a retryable failure of the claimed delivery performs.
pub(super) fn schedule_call(delay: Duration) -> PersistenceCall {
    PersistenceCall::Schedule {
        task_id: TASK_ID.to_owned(),
        worker_id: WORKER.to_owned(),
        claim_token: CLAIM_TOKEN.to_owned(),
        attempt: ATTEMPT,
        delay,
    }
}
