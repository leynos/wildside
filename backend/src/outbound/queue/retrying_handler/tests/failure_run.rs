//! Shared harness that drives one failed delivery through the wrapper.
//!
//! The effect modules assert which writes a single failed delivery produces.
//! They all need the same fixed attempt, the same recording doubles, and the
//! same expectation that the wrapper returns an error, so those live here
//! rather than being restated in each module.

use std::num::NonZeroU32;

use super::super::{RetryFailureContext, handle_failure};
use super::doubles::{
    FixedJitter, PersistenceCall, QUEUE, RecordingMetrics, RecordingPersistence, TASK_ID, WORKER,
    WriteOutcome,
};
use super::policy;
use crate::domain::jobs::retry::{JitterSample, JobErrorKind, JobFailure};
use crate::domain::ports::{RouteDeadLetterOutcome, RouteRetryOutcome};
use crate::outbound::queue::TerminalRetryReason;

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
    /// The wrapper always asks Apalis to keep the task failed, so the error is
    /// expected and deliberately dropped here.
    pub(super) async fn apply(&self, kind: JobErrorKind) {
        self.apply_with_identity(kind, Some(TASK_ID), Some(WORKER))
            .await;
    }

    /// Drive one failed delivery with a chosen task and worker identity.
    ///
    /// The scheduler is only reachable when both identities are present; a
    /// `None` on either side exercises the wrapper's refusal to persist.
    pub(super) async fn apply_with_identity(
        &self,
        kind: JobErrorKind,
        task_id: Option<&'static str>,
        worker_id: Option<&'static str>,
    ) {
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

/// The terminal write a fatal failure of the shared delivery performs.
pub(super) fn terminal_call() -> PersistenceCall {
    PersistenceCall::Terminal {
        task_id: TASK_ID.to_owned(),
        worker_id: WORKER.to_owned(),
        attempt: ATTEMPT,
        reason: TerminalRetryReason::Fatal,
        shrink_limit: true,
    }
}

/// The cap-lowering fallback the shared delivery performs.
pub(super) fn clamp_call() -> PersistenceCall {
    PersistenceCall::Clamp {
        task_id: TASK_ID.to_owned(),
        worker_id: WORKER.to_owned(),
        attempt: ATTEMPT,
    }
}
