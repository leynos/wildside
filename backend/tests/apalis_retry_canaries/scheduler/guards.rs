//! PostgreSQL canaries for the retry scheduler's guarded writes.
//!
//! Every guarded statement matches the row's status, recorded lock owner, and
//! attempt count, so a row that has completed, been re-locked, or been
//! delivered again refuses it. These canaries cover each statement against
//! each refusal cause, and pin the terminal-cap fallback that applies when a
//! terminal write has already failed.

use backend::outbound::queue::{
    PostgresRetryScheduler, RetryPersistence, RetryScheduleUpdate, TerminalCapClamp,
    TerminalRetryReason,
};

use super::{
    CALLER, Canary, DEADLINE_SQL, DELIVERED_ATTEMPT, OTHER_WORKER, OWNER, RETRY_DELAY,
    STORED_ATTEMPTS, SchedulerRow, delivery_attempt, terminal_update,
};

/// The guarded write a refusal canary attempts.
#[derive(Clone, Copy, Debug)]
enum GuardedWrite {
    /// Move the retry deadline.
    ScheduleRetry,
    /// Record a terminal reason.
    TerminalFailure,
    /// Lower the attempt cap after a failed terminal write.
    ClampCap,
}

/// Why a row must refuse a guarded write.
#[derive(Clone, Copy, Debug)]
enum RefusalCause {
    /// The row is already `Done`, so no worker may change it.
    NotRunning,
    /// Another worker holds the lock.
    ForeignOwner,
    /// The row has been delivered again since this delivery locked it.
    StaleDelivery,
}

impl RefusalCause {
    /// The row's status, recorded lock owner, and attempt count.
    fn row(self) -> (&'static str, &'static str, i32) {
        match self {
            Self::NotRunning => ("Done", OWNER, STORED_ATTEMPTS),
            Self::ForeignOwner => ("Running", OTHER_WORKER, STORED_ATTEMPTS),
            // The re-delivery advanced the counter the stale write still
            // believes it matches; only that column separates the two.
            Self::StaleDelivery => ("Running", OWNER, STORED_ATTEMPTS + 1),
        }
    }

    /// The worker that attempts the write.
    fn caller(self) -> &'static str {
        match self {
            Self::NotRunning => OWNER,
            Self::ForeignOwner => CALLER,
            Self::StaleDelivery => OWNER,
        }
    }
}

/// Every refusal route ends in the same observable state, so one canary states
/// the invariant for every combination of write and cause.
#[rstest::rstest]
#[case::schedule_on_completed_row(GuardedWrite::ScheduleRetry, RefusalCause::NotRunning)]
#[case::schedule_under_foreign_lock(GuardedWrite::ScheduleRetry, RefusalCause::ForeignOwner)]
#[case::schedule_from_a_stale_delivery(GuardedWrite::ScheduleRetry, RefusalCause::StaleDelivery)]
#[case::terminal_on_completed_row(GuardedWrite::TerminalFailure, RefusalCause::NotRunning)]
#[case::terminal_under_foreign_lock(GuardedWrite::TerminalFailure, RefusalCause::ForeignOwner)]
#[case::terminal_from_a_stale_delivery(GuardedWrite::TerminalFailure, RefusalCause::StaleDelivery)]
#[case::clamp_on_completed_row(GuardedWrite::ClampCap, RefusalCause::NotRunning)]
#[case::clamp_under_foreign_lock(GuardedWrite::ClampCap, RefusalCause::ForeignOwner)]
#[case::clamp_from_a_stale_delivery(GuardedWrite::ClampCap, RefusalCause::StaleDelivery)]
#[test]
fn guarded_write_refuses_a_row_it_may_not_change(
    #[case] write: GuardedWrite,
    #[case] cause: RefusalCause,
) {
    let (status, owner, attempts) = cause.row();
    let caller = cause.caller();
    let canary = Canary::start(
        SchedulerRow {
            status,
            attempts,
            ..SchedulerRow::running("canary.scheduler-refused", owner)
        },
        Some(serde_json::json!({ "keep": "marker" })),
    );
    let scheduler = || PostgresRetryScheduler::new(canary.pool.clone());

    match write {
        GuardedWrite::ScheduleRetry => canary.assert_write_is_refused(
            |canary| canary.read_micros(DEADLINE_SQL, "deadline"),
            || {
                canary
                    .environment
                    .run(scheduler().schedule_retry(RetryScheduleUpdate {
                        task_id: &canary.id,
                        worker_id: caller,
                        attempt: delivery_attempt(),
                        delay: RETRY_DELAY,
                    }))
                    .expect("the guarded update executes")
            },
            "a row the caller may not change must not accept a retry deadline",
        ),
        GuardedWrite::TerminalFailure => canary.assert_write_is_refused(
            |canary| canary.read_terminal_state(),
            || {
                canary
                    .environment
                    .run(scheduler().record_terminal_failure(terminal_update(
                        &canary.id,
                        caller,
                        TerminalRetryReason::Exhausted,
                        true,
                    )))
                    .expect("the guarded update executes")
            },
            "a row the caller may not change must not accept a terminal reason",
        ),
        GuardedWrite::ClampCap => canary.assert_write_is_refused(
            |canary| canary.read_terminal_state(),
            || {
                canary
                    .environment
                    .run(scheduler().clamp_terminal_cap(TerminalCapClamp {
                        task_id: &canary.id,
                        worker_id: caller,
                        attempt: delivery_attempt(),
                    }))
                    .expect("the guarded update executes")
            },
            "a row the caller may not change must not accept a lowered attempt cap",
        ),
    }
}

/// Scenario: the fallback runs against a row this delivery still owns.
///
/// Invariant: the cap falls to the terminating attempt, so the `Failed` row
/// Apalis records next is not fetched again.
#[test]
fn cap_fallback_lowers_the_cap_to_the_delivered_attempt() {
    let canary = Canary::start(SchedulerRow::running("canary.cap-fallback", OWNER), None);

    let clamped = canary
        .environment
        .run(
            PostgresRetryScheduler::new(canary.pool.clone()).clamp_terminal_cap(TerminalCapClamp {
                task_id: &canary.id,
                worker_id: OWNER,
                attempt: delivery_attempt(),
            }),
        )
        .expect("the guarded update executes");

    assert!(clamped, "the recorded lock owner still owns a running row");
    assert_eq!(
        canary.read_terminal_state().max_attempts,
        i32::try_from(DELIVERED_ATTEMPT).expect("the delivered attempt is an i32"),
        "the cap must fall to the attempt that terminated the task"
    );
}

/// Scenario: the fallback runs against a row whose cap is already lower.
///
/// Invariant: the write never raises a cap, so a fallback that runs out of
/// order cannot make an exhausted row fetchable again.
#[test]
fn cap_fallback_never_raises_an_already_lower_cap() {
    let mut row = SchedulerRow::running("canary.cap-fallback-lower", OWNER);
    row.max_attempts = STORED_ATTEMPTS;
    let canary = Canary::start(row, None);

    let clamped = canary
        .environment
        .run(
            PostgresRetryScheduler::new(canary.pool.clone()).clamp_terminal_cap(TerminalCapClamp {
                task_id: &canary.id,
                worker_id: OWNER,
                attempt: delivery_attempt(),
            }),
        )
        .expect("the guarded update executes");

    assert!(clamped, "the recorded lock owner still owns a running row");
    assert_eq!(
        canary.read_terminal_state().max_attempts,
        STORED_ATTEMPTS,
        "a cap that is already lower must be left alone"
    );
}
