//! Delivery values the guarded retry writes bind.
//!
//! Each value groups the fields that describe one delivery. Passing them
//! positionally would let a call site pair a worker with another task's
//! attempt, or with a token from a generation that no longer owns the row,
//! which is exactly the write the delivery guard exists to refuse.

use std::num::NonZeroU32;
use std::time::Duration;

use crate::domain::ports::DeadReason;

/// Details persisted when a task reaches a terminal retry outcome.
///
/// The port accepts this value rather than a positional argument list so a
/// call site cannot transpose the columns that describe one row.
pub struct TerminalRetryUpdate<'a> {
    /// Identity of the task that terminated.
    pub task_id: &'a str,
    /// Worker that owns the task's lock.
    pub worker_id: &'a str,
    /// Claim token the terminating delivery was locked under.
    ///
    /// The row must still hold this token. A delivery that was never claimed
    /// carries the empty string, which matches no row, exactly as the
    /// acknowledgement treats a missing token.
    pub claim_token: &'a str,
    /// Attempt that terminated the task.
    pub attempt: NonZeroU32,
    /// Why the task will not be retried.
    pub reason: DeadReason,
    /// Whether the attempt limit shrinks to the terminating attempt.
    pub shrink_limit: bool,
}

/// Details of one delivery's retry to schedule.
///
/// The fields describe a single delivery, so they travel as one value: passing
/// them positionally would let a call site pair a worker with the wrong task's
/// attempt, which is exactly the write the delivery guard exists to refuse.
pub struct RetryScheduleUpdate<'a> {
    /// Identity of the task to retry.
    pub task_id: &'a str,
    /// Worker that owns the task's lock.
    pub worker_id: &'a str,
    /// Claim token the failing delivery was locked under.
    ///
    /// The row must still hold this token. A delivery that was never claimed
    /// carries the empty string, which matches no row, exactly as the
    /// acknowledgement treats a missing token.
    pub claim_token: &'a str,
    /// Attempt that just failed, as the delivery counts it.
    pub attempt: NonZeroU32,
    /// Delay before the task becomes eligible again.
    pub delay: Duration,
}

/// Details of the fallback write that lowers a row's attempt cap.
///
/// The write exists for one case: a terminal classification whose reason could
/// not be stored. Apalis still acknowledges that delivery as failed, and a
/// failed row stays fetchable while `attempts < max_attempts`, so the cap is
/// lowered to the terminating attempt before that acknowledgement lands.
#[derive(Clone, Copy)]
pub struct TerminalCapClamp<'a> {
    /// Identity of the task that terminated.
    pub task_id: &'a str,
    /// Worker that owns the task's lock.
    pub worker_id: &'a str,
    /// Claim token the terminating delivery was locked under.
    ///
    /// The row must still hold this token. A delivery that was never claimed
    /// carries the empty string, which matches no row, exactly as the
    /// acknowledgement treats a missing token.
    pub claim_token: &'a str,
    /// Attempt that terminated the task.
    pub attempt: NonZeroU32,
}
