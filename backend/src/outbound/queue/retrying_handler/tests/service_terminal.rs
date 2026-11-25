//! Terminal-write tests for the retry wrapper's persistence contract.
//!
//! A terminal decision asks for one guarded terminal write. Only an applied
//! write counts as a dead letter; a refused or failed write increments the
//! terminal-write-failure counter instead, and a failed write is followed by a
//! cap-lowering fallback that is retried a bounded number of times.

use super::doubles::{PersistenceCall, WriteOutcome};
use super::failure_run::{FailureRun, clamp_call, terminal_call};
use crate::domain::jobs::retry::JobErrorKind;
use crate::domain::ports::RouteDeadLetterOutcome;
use tracing_test::traced_test;

#[tokio::test]
async fn fatal_failure_records_one_terminal_write() {
    let run = FailureRun::new(WriteOutcome::Updated, WriteOutcome::Updated);

    run.apply(JobErrorKind::Fatal).await;

    assert_eq!(
        run.calls(),
        vec![terminal_call()],
        "a terminal decision that applies is the delivery's only write"
    );
    assert_eq!(run.dead_letters(), vec![RouteDeadLetterOutcome::Fatal]);
    assert!(run.retries().is_empty());
}

/// Scenario: a fatal failure arrives from a delivery that never passed through
/// the lock step.
///
/// Invariant: the terminal write binds the empty string rather than a stale or
/// invented token. Kept apart from the claimed-delivery cases so a wrapper that
/// dropped the token from every request could not pass them by coincidence.
#[tokio::test]
async fn a_tokenless_terminal_delivery_binds_the_empty_string() {
    let run = FailureRun::new(WriteOutcome::Updated, WriteOutcome::Updated);

    run.apply_without_claim_token(JobErrorKind::Fatal).await;

    match run.calls().as_slice() {
        [PersistenceCall::Terminal { claim_token, .. }] => assert_eq!(
            claim_token, "",
            "a delivery with no claim token must bind the empty string"
        ),
        other => panic!("expected exactly one terminal write, got {other:?}"),
    }
}

#[tokio::test]
async fn terminal_write_failure_records_its_own_outcome_only() {
    let run = FailureRun::new(WriteOutcome::Updated, WriteOutcome::NotOwned);

    run.apply(JobErrorKind::Fatal).await;

    assert_eq!(
        run.calls(),
        vec![terminal_call()],
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
    let run = FailureRun::failing_terminal_write([WriteOutcome::Updated]);

    run.apply(JobErrorKind::Fatal).await;

    assert_eq!(
        run.calls(),
        vec![terminal_call(), clamp_call()],
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
async fn a_transient_cap_fallback_failure_is_retried() {
    let run = FailureRun::failing_terminal_write([WriteOutcome::Failed, WriteOutcome::Updated]);

    run.apply(JobErrorKind::Fatal).await;

    assert_eq!(
        run.calls(),
        vec![terminal_call(), clamp_call(), clamp_call()],
        "a fallback that fails once must be retried before the delivery unwinds"
    );
    assert!(
        logs_contain("terminal_cap_clamped"),
        "the retried fallback must report that it applied"
    );
}

#[tokio::test]
async fn a_refused_cap_fallback_is_not_retried() {
    let run = FailureRun::failing_terminal_write([WriteOutcome::NotOwned, WriteOutcome::Updated]);

    run.apply(JobErrorKind::Fatal).await;

    assert_eq!(
        run.calls(),
        vec![terminal_call(), clamp_call()],
        "a refused guard means another actor owns the row, so a retry would repeat its decision"
    );
}

#[tokio::test]
async fn cap_fallback_exhaustion_retries_a_bounded_number_of_times() {
    let run = FailureRun::failing_terminal_write([WriteOutcome::Failed]);

    run.apply(JobErrorKind::Fatal).await;

    assert_eq!(
        run.calls(),
        vec![terminal_call(), clamp_call(), clamp_call(), clamp_call()],
        "the fallback must be attempted three times, each carrying the claim token"
    );
}

#[traced_test]
#[tokio::test]
async fn exhausted_cap_fallback_raises_the_alarm() {
    let run = FailureRun::failing_terminal_write([WriteOutcome::Failed]);

    run.apply(JobErrorKind::Fatal).await;

    assert!(
        logs_contain("terminal_cap_clamp_exhausted"),
        "a retired job left unprotected must raise the alarm"
    );
    assert!(
        !logs_contain("terminal_cap_clamped"),
        "no fallback applied, so none may be reported as clamped"
    );
}
