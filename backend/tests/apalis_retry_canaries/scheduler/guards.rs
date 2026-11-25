//! PostgreSQL canaries for the retry scheduler's guarded writes.
//!
//! Every guarded statement matches the row's status, recorded lock owner,
//! attempt count, and claim token, so a completed, re-locked, re-delivered, or
//! later-claimed row refuses it. These canaries cover each statement against
//! each refusal cause, pin the terminal-cap fallback, and drive a dead-letter
//! requeue and re-claim to show that only the token separates generations.

use std::num::NonZeroU32;

use backend::domain::ports::DeadReason;
use backend::outbound::queue::{
    PostgresRetryScheduler, RetryPersistence, RetryScheduleUpdate, TerminalCapClamp,
    TerminalRetryUpdate,
};

use super::{
    CALLER, CLAIM_TOKEN, Canary, DEADLINE_SQL, DELIVERED_ATTEMPT, OTHER_WORKER, OWNER, RETRY_DELAY,
    STORED_ATTEMPTS, SchedulerRow, delivery_attempt, terminal_update,
};
use crate::ack_guard::{claim_row, requeue_dead_job};
use crate::fixtures::{GuardedRowState, JobId, QueueName, read_guarded_row};
use crate::support::apalis_job_id::apalis_job_id;

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
                        claim_token: CLAIM_TOKEN,
                        attempt: delivery_attempt(),
                        delay: RETRY_DELAY,
                    }))
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
                        DeadReason::Exhausted,
                        true,
                    )))
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
                        claim_token: CLAIM_TOKEN,
                        attempt: delivery_attempt(),
                    }))
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
                claim_token: CLAIM_TOKEN,
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
                claim_token: CLAIM_TOKEN,
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

/// The worker both the stale and the replacement deliveries run under.
const REQUEUED_WORKER: &str = "canary-scheduler-requeued-worker";

/// The claim token the generation the requeue replaced stamped on the row.
const REQUEUED_STALE_CLAIM: &str = "canary-scheduler-requeued-stale";

/// The attempt both generations' first deliveries report.
///
/// A requeue resets the stored count to zero, so each generation counts one
/// against a row holding zero and the count no longer separates them.
const REQUEUED_DELIVERED_ATTEMPT: u32 = 1;

/// The stored attempt the stale delivery's predicate expects to match.
fn requeued_stored_attempt() -> i32 {
    i32::try_from(REQUEUED_DELIVERED_ATTEMPT - 1).expect("the stored attempt is an i32")
}

/// The attempt the stale delivery and the replacement both report.
fn requeued_attempt() -> NonZeroU32 {
    NonZeroU32::new(REQUEUED_DELIVERED_ATTEMPT).expect("the requeued delivery counts one")
}

/// Insert a dead row, requeue it, and claim it again through production code.
///
/// The returned token is the one the replacement claim stamped on the row.
fn claimed_replacement() -> (Canary, String) {
    let queue_name = format!("canary.scheduler-requeued-{}", apalis_job_id());
    let canary = Canary::start(
        SchedulerRow {
            status: "Killed",
            run_at_offset_seconds: -1.0,
            claim_token: REQUEUED_STALE_CLAIM,
            ..SchedulerRow::running(&queue_name, REQUEUED_WORKER)
        },
        None,
    );
    requeue_dead_job(
        canary.environment,
        QueueName(canary.queue.as_str()),
        JobId(canary.id.as_str()),
    );
    let token = claim_row(
        canary.environment,
        REQUEUED_WORKER,
        QueueName(canary.queue.as_str()),
        JobId(canary.id.as_str()),
    );
    (canary, token)
}

/// Assert only the token separates the replacement from the stale delivery.
fn assert_only_the_token_separates(state: &GuardedRowState, replacement_token: &str) {
    assert_eq!(
        state.status, "Running",
        "the replacement claim promotes the requeued row"
    );
    assert_eq!(
        state.attempts,
        requeued_stored_attempt(),
        "the requeue resets the count to the value the stale delivery matches"
    );
    assert_eq!(
        state.lock_by.as_deref(),
        Some(REQUEUED_WORKER),
        "the replacement claim records the same worker name the stale delivery carries"
    );
    assert_eq!(
        state.claim_token.as_deref(),
        Some(replacement_token),
        "the row carries the token the replacement claim stamped"
    );
    assert_ne!(
        state.claim_token.as_deref(),
        Some(REQUEUED_STALE_CLAIM),
        "every claim mints a token that never repeats"
    );
}

/// Build a terminal update for the requeued generation's delivery.
fn requeued_terminal_update<'a>(id: &'a str, claim_token: &'a str) -> TerminalRetryUpdate<'a> {
    TerminalRetryUpdate {
        task_id: id,
        worker_id: REQUEUED_WORKER,
        claim_token,
        attempt: requeued_attempt(),
        reason: DeadReason::Exhausted,
        shrink_limit: true,
    }
}

/// Build the cap-clamp update for the requeued generation's delivery.
fn requeued_clamp<'a>(id: &'a str, claim_token: &'a str) -> TerminalCapClamp<'a> {
    TerminalCapClamp {
        task_id: id,
        worker_id: REQUEUED_WORKER,
        claim_token,
        attempt: requeued_attempt(),
    }
}

/// Assert the replacement's applied write changed the column it owns.
///
/// This ties the `Ok(true)` result to the stored effect a matched write has.
fn assert_replacement_effect(write: GuardedWrite, before: &GuardedRowState, canary: &Canary) {
    let after = read_guarded_row(&canary.environment.runtime, &canary.pool, &canary.id);
    match write {
        GuardedWrite::ScheduleRetry => assert_ne!(
            after.run_at_micros, before.run_at_micros,
            "the replacement's retry write must move the deadline"
        ),
        GuardedWrite::TerminalFailure | GuardedWrite::ClampCap => assert_eq!(
            after.max_attempts,
            i32::try_from(REQUEUED_DELIVERED_ATTEMPT).expect("the delivered attempt is an i32"),
            "the replacement's write must store its delivered attempt as the cap"
        ),
    }
}

/// Issue one guarded write for the requeued row under `claim_token`.
///
/// The identifier, worker, and attempt are the same for both generations, so
/// only the token distinguishes the stale delivery from the replacement.
fn issue_requeued_write(
    write: GuardedWrite,
    canary: &Canary,
    claim_token: &str,
) -> Result<bool, sqlx::Error> {
    let scheduler = PostgresRetryScheduler::new(canary.pool.clone());
    let environment = canary.environment;
    match write {
        GuardedWrite::ScheduleRetry => {
            environment.run(scheduler.schedule_retry(RetryScheduleUpdate {
                task_id: &canary.id,
                worker_id: REQUEUED_WORKER,
                claim_token,
                attempt: requeued_attempt(),
                delay: RETRY_DELAY,
            }))
        }
        GuardedWrite::TerminalFailure => environment.run(
            scheduler.record_terminal_failure(requeued_terminal_update(&canary.id, claim_token)),
        ),
        GuardedWrite::ClampCap => {
            environment.run(scheduler.clamp_terminal_cap(requeued_clamp(&canary.id, claim_token)))
        }
    }
}

/// Scenario: a dead-letter requeue reset the attempt count and the same worker
/// re-claimed the row while the stale delivery was still active.
///
/// Invariant: the stale delivery's guarded write matches no row even though its
/// identifier, worker, `Running` status, and stored attempt count all match,
/// because its claim token belongs to the generation the requeue replaced. The
/// replacement's own write still applies; only the token separates them.
#[rstest::rstest]
#[case::schedule_retry(GuardedWrite::ScheduleRetry)]
#[case::record_terminal_failure(GuardedWrite::TerminalFailure)]
#[case::clamp_terminal_cap(GuardedWrite::ClampCap)]
#[test]
fn a_requeued_row_refuses_its_previous_deliveries_guarded_write(#[case] write: GuardedWrite) {
    let (canary, token) = claimed_replacement();
    let before = read_guarded_row(&canary.environment.runtime, &canary.pool, &canary.id);
    assert_only_the_token_separates(&before, &token);

    let stale = issue_requeued_write(write, &canary, REQUEUED_STALE_CLAIM);
    assert!(
        matches!(&stale, Ok(false)),
        "{write:?} from the previous generation must match no row: {stale:?}"
    );
    assert_eq!(
        read_guarded_row(&canary.environment.runtime, &canary.pool, &canary.id),
        before,
        "a refused {write:?} must leave the row untouched"
    );

    let owned = issue_requeued_write(write, &canary, &token);
    assert!(
        matches!(&owned, Ok(true)),
        "the replacement's own {write:?} must still apply: {owned:?}"
    );
    assert_replacement_effect(write, &before, &canary);
}
