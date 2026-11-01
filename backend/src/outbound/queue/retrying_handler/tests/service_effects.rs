//! Side-effect tests for the retry wrapper's persistence contract.
//!
//! The wrapper must perform exactly one guarded write per failed delivery and
//! no write at all after a successful handler. The scheduler is behind
//! [`RetryPersistence`], so these tests observe calls directly instead of
//! inferring them from database state.

use std::num::NonZeroU32;

use super::super::{RetryFailureContext, handle_failure};
use super::doubles::{
    PersistenceCall, QUEUE, RecordingMetrics, RecordingPersistence, TASK_ID, WORKER, WriteOutcome,
};
use super::policy;
use crate::domain::jobs::retry::{JobErrorKind, JobFailure};
use crate::domain::ports::{RouteDeadLetterOutcome, RouteRetryOutcome};
use crate::outbound::queue::TerminalRetryReason;
use tracing_test::traced_test;

fn failure(kind: JobErrorKind) -> JobFailure {
    JobFailure::new(kind, "probe failure")
}

async fn apply_failure(
    persistence: &RecordingPersistence,
    metrics: &RecordingMetrics,
    kind: JobErrorKind,
    attempt: u32,
    context_max_attempts: u32,
) {
    let retry_policy = policy(3);
    let context = RetryFailureContext {
        attempt: NonZeroU32::new(attempt).expect("positive attempt"),
        context_max_attempts,
        policy: &retry_policy,
        scheduler: persistence,
        metrics,
        queue_name: QUEUE,
        task_id: Some(TASK_ID),
        worker_id: Some(WORKER),
    };
    handle_failure(failure(kind), &context)
        .await
        .expect_err("the wrapper asks Apalis to keep the task failed");
}

#[tokio::test]
async fn retryable_failure_schedules_exactly_one_write() {
    let persistence = RecordingPersistence::new(WriteOutcome::Updated, WriteOutcome::Updated);
    let metrics = RecordingMetrics::default();

    apply_failure(&persistence, &metrics, JobErrorKind::Retryable, 1, 5).await;

    assert_eq!(
        persistence.calls(),
        vec![PersistenceCall::Schedule {
            task_id: TASK_ID.to_owned(),
            worker_id: WORKER.to_owned(),
            delay: std::time::Duration::from_millis(10),
        }]
    );
    assert_eq!(metrics.retries(), vec![RouteRetryOutcome::Scheduled]);
    assert!(metrics.dead_letters().is_empty());
}

#[tokio::test]
async fn schedule_failure_counts_one_outcome_and_stops_writing() {
    let persistence = RecordingPersistence::new(WriteOutcome::NotOwned, WriteOutcome::Updated);
    let metrics = RecordingMetrics::default();

    apply_failure(&persistence, &metrics, JobErrorKind::Retryable, 1, 5).await;

    assert_eq!(persistence.calls().len(), 1, "no retry write after failure");
    assert_eq!(metrics.retries(), vec![RouteRetryOutcome::ScheduleFailure]);
}

#[tokio::test]
async fn scheduling_error_counts_one_outcome_and_stops_writing() {
    let persistence = RecordingPersistence::new(WriteOutcome::Failed, WriteOutcome::Updated);
    let metrics = RecordingMetrics::default();

    apply_failure(&persistence, &metrics, JobErrorKind::Retryable, 1, 5).await;

    assert_eq!(persistence.calls().len(), 1, "no retry write after error");
    assert_eq!(metrics.retries(), vec![RouteRetryOutcome::ScheduleFailure]);
}

#[traced_test]
#[tokio::test]
async fn schedule_failure_warns_with_the_failure_decision() {
    let persistence = RecordingPersistence::new(WriteOutcome::NotOwned, WriteOutcome::Updated);
    let metrics = RecordingMetrics::default();

    apply_failure(&persistence, &metrics, JobErrorKind::Retryable, 1, 5).await;

    assert!(logs_contain("schedule_failure"));
}

#[tokio::test]
async fn fatal_failure_records_one_terminal_write() {
    let persistence = RecordingPersistence::new(WriteOutcome::Updated, WriteOutcome::Updated);
    let metrics = RecordingMetrics::default();

    apply_failure(&persistence, &metrics, JobErrorKind::Fatal, 1, 5).await;

    assert_eq!(
        persistence.calls(),
        vec![PersistenceCall::Terminal {
            task_id: TASK_ID.to_owned(),
            worker_id: WORKER.to_owned(),
            attempt: 1,
            reason: TerminalRetryReason::Fatal,
            shrink_limit: true,
        }]
    );
    assert_eq!(metrics.dead_letters(), vec![RouteDeadLetterOutcome::Fatal]);
    assert!(metrics.retries().is_empty());
}

#[tokio::test]
async fn terminal_write_failure_records_no_dead_letter_outcome() {
    let persistence = RecordingPersistence::new(WriteOutcome::Updated, WriteOutcome::NotOwned);
    let metrics = RecordingMetrics::default();

    apply_failure(&persistence, &metrics, JobErrorKind::Fatal, 1, 5).await;

    assert_eq!(persistence.calls().len(), 1);
    assert!(metrics.dead_letters().is_empty());
}
