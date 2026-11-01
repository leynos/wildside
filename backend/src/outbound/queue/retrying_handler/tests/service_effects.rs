//! Side-effect tests for the retry wrapper's persistence contract.
//!
//! The wrapper must perform exactly one guarded write per failed delivery and
//! no write at all after a successful handler. The scheduler is behind
//! [`RetryPersistence`], so these tests observe calls directly instead of
//! inferring them from database state.

use std::num::NonZeroU32;
use std::time::Duration;

use super::super::{RetryFailureContext, handle_failure};
use super::doubles::{
    PersistenceCall, QUEUE, RecordingMetrics, RecordingPersistence, TASK_ID, WORKER, WriteOutcome,
};
use super::policy;
use crate::domain::jobs::retry::{JobErrorKind, JobFailure};
use crate::domain::ports::{RouteDeadLetterOutcome, RouteRetryOutcome};
use crate::outbound::queue::TerminalRetryReason;
use tracing_test::traced_test;

/// Delivery attempt every case in this module exercises.
///
/// The wrapper's decision table is covered elsewhere; these tests only care
/// which write a single failed delivery produces, so the attempt and the
/// task's own attempt limit are held fixed.
const ATTEMPT: u32 = 1;
const CONTEXT_MAX_ATTEMPTS: u32 = 5;

/// One failed delivery, observed through recording doubles.
struct FailureRun {
    persistence: RecordingPersistence,
    metrics: RecordingMetrics,
}

impl FailureRun {
    fn new(schedule: WriteOutcome, terminal: WriteOutcome) -> Self {
        Self {
            persistence: RecordingPersistence::new(schedule, terminal),
            metrics: RecordingMetrics::default(),
        }
    }

    /// Drive one failed delivery through the wrapper's failure path.
    ///
    /// The wrapper always asks Apalis to keep the task failed, so the error is
    /// expected and deliberately dropped here.
    async fn apply(&self, kind: JobErrorKind) {
        let retry_policy = policy(3);
        let context = RetryFailureContext {
            attempt: NonZeroU32::new(ATTEMPT).expect("positive attempt"),
            context_max_attempts: CONTEXT_MAX_ATTEMPTS,
            policy: &retry_policy,
            scheduler: &self.persistence,
            metrics: &self.metrics,
            queue_name: QUEUE,
            task_id: Some(TASK_ID),
            worker_id: Some(WORKER),
        };
        let failure = JobFailure::new(kind, "probe failure");
        handle_failure(failure, &context)
            .await
            .expect_err("the wrapper asks Apalis to keep the task failed");
    }

    fn calls(&self) -> Vec<PersistenceCall> {
        self.persistence.calls()
    }

    fn retries(&self) -> Vec<RouteRetryOutcome> {
        self.metrics.retries()
    }

    fn dead_letters(&self) -> Vec<RouteDeadLetterOutcome> {
        self.metrics.dead_letters()
    }
}

#[tokio::test]
async fn retryable_failure_schedules_exactly_one_write() {
    let run = FailureRun::new(WriteOutcome::Updated, WriteOutcome::Updated);

    run.apply(JobErrorKind::Retryable).await;

    assert_eq!(
        run.calls(),
        vec![PersistenceCall::Schedule {
            task_id: TASK_ID.to_owned(),
            worker_id: WORKER.to_owned(),
            delay: Duration::from_millis(10),
        }]
    );
    assert_eq!(run.retries(), vec![RouteRetryOutcome::Scheduled]);
    assert!(run.dead_letters().is_empty());
}

#[tokio::test]
async fn schedule_failure_counts_one_outcome_and_stops_writing() {
    let run = FailureRun::new(WriteOutcome::NotOwned, WriteOutcome::Updated);

    run.apply(JobErrorKind::Retryable).await;

    assert_eq!(run.calls().len(), 1, "no retry write after failure");
    assert_eq!(run.retries(), vec![RouteRetryOutcome::ScheduleFailure]);
}

#[tokio::test]
async fn scheduling_error_counts_one_outcome_and_stops_writing() {
    let run = FailureRun::new(WriteOutcome::Failed, WriteOutcome::Updated);

    run.apply(JobErrorKind::Retryable).await;

    assert_eq!(run.calls().len(), 1, "no retry write after error");
    assert_eq!(run.retries(), vec![RouteRetryOutcome::ScheduleFailure]);
}

#[traced_test]
#[tokio::test]
async fn schedule_failure_warns_with_the_failure_decision() {
    let run = FailureRun::new(WriteOutcome::NotOwned, WriteOutcome::Updated);

    run.apply(JobErrorKind::Retryable).await;

    assert!(logs_contain("schedule_failure"));
}

#[tokio::test]
async fn fatal_failure_records_one_terminal_write() {
    let run = FailureRun::new(WriteOutcome::Updated, WriteOutcome::Updated);

    run.apply(JobErrorKind::Fatal).await;

    assert_eq!(
        run.calls(),
        vec![PersistenceCall::Terminal {
            task_id: TASK_ID.to_owned(),
            worker_id: WORKER.to_owned(),
            attempt: ATTEMPT,
            reason: TerminalRetryReason::Fatal,
            shrink_limit: true,
        }]
    );
    assert_eq!(run.dead_letters(), vec![RouteDeadLetterOutcome::Fatal]);
    assert!(run.retries().is_empty());
}

#[tokio::test]
async fn terminal_write_failure_records_no_dead_letter_outcome() {
    let run = FailureRun::new(WriteOutcome::Updated, WriteOutcome::NotOwned);

    run.apply(JobErrorKind::Fatal).await;

    assert_eq!(run.calls().len(), 1);
    assert!(run.dead_letters().is_empty());
}
