//! Terminal-write tests for the retry wrapper's persistence contract.
//!
//! A terminal decision asks for one guarded terminal write. Only an applied
//! write counts as a dead letter; a refused or failed write increments the
//! terminal-write-failure counter instead, and a failed write is followed by
//! exactly one cap-lowering fallback.

use super::doubles::{
    PersistenceCall, RecordingMetrics, RecordingPersistence, TASK_ID, WORKER, WriteOutcome,
};
use super::failure_run::{ATTEMPT, FailureRun};
use crate::domain::jobs::retry::JobErrorKind;
use crate::domain::ports::RouteDeadLetterOutcome;
use crate::outbound::queue::TerminalRetryReason;
use tracing_test::traced_test;

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
async fn terminal_write_failure_records_its_own_outcome_only() {
    let run = FailureRun::new(WriteOutcome::Updated, WriteOutcome::NotOwned);

    run.apply(JobErrorKind::Fatal).await;

    assert!(
        matches!(run.calls().as_slice(), [PersistenceCall::Terminal { .. }]),
        "a refused terminal write is not followed by a fallback: the row is not ours"
    );
    assert!(
        run.dead_letters().is_empty(),
        "an unwritten terminal decision is not a dead letter"
    );
    assert_eq!(
        run.terminal_write_failures(),
        vec![RouteDeadLetterOutcome::Fatal],
        "a refused terminal write has its own bounded counter"
    );
}

#[tokio::test]
async fn terminal_write_error_lowers_the_cap_after_the_failed_write() {
    let run = FailureRun::new(WriteOutcome::Updated, WriteOutcome::Failed);

    run.apply(JobErrorKind::Fatal).await;

    assert_eq!(
        run.calls(),
        vec![
            PersistenceCall::Terminal {
                task_id: TASK_ID.to_owned(),
                worker_id: WORKER.to_owned(),
                attempt: ATTEMPT,
                reason: TerminalRetryReason::Fatal,
                shrink_limit: true,
            },
            PersistenceCall::Clamp {
                task_id: TASK_ID.to_owned(),
                worker_id: WORKER.to_owned(),
                attempt: ATTEMPT,
            },
        ],
        "a failed terminal write must be followed by one cap-lowering fallback"
    );
    assert!(
        run.dead_letters().is_empty(),
        "a failed terminal write is not a dead letter"
    );
    assert_eq!(
        run.terminal_write_failures(),
        vec![RouteDeadLetterOutcome::Fatal]
    );
}

#[traced_test]
#[tokio::test]
async fn failed_cap_fallback_logs_its_own_decision() {
    let run = FailureRun {
        persistence: RecordingPersistence::new(WriteOutcome::Updated, WriteOutcome::Failed)
            .with_clamp(WriteOutcome::Failed),
        metrics: RecordingMetrics::default(),
    };

    run.apply(JobErrorKind::Fatal).await;

    assert!(logs_contain("terminal_cap_clamp_failed"));
}
