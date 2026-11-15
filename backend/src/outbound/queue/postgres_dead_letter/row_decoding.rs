//! Conversion of stored dead-letter rows into domain values.
//!
//! Every function here is pure: a row that the domain cannot represent is
//! rejected with the field that made it unreadable, and the caller decides
//! whether, and where, to report it. Keeping the conversion side-effect free
//! is what lets the listing contract be tested without inspecting logs.

use crate::domain::ports::{DeadJob, DeadJobId, DeadJobSkip, DeadReason, SkippedDeadJob};

use super::DeadJobRow;

/// Longest prefix of a stored error message a listed job exposes.
const MAX_LAST_ERROR_BYTES: usize = 1024;

/// Convert a stored row, rejecting it when its attempt or requeue metadata is
/// invalid rather than surfacing a job the domain cannot represent.
///
/// The rejection names the offending field and the row, and does no logging of
/// its own, so the listing can report it once the whole snapshot is known.
pub(super) fn dead_job_from_row(row: DeadJobRow) -> Result<DeadJob, SkippedDeadJob> {
    let id = DeadJobId::new(row.id);
    let attempts = u32::try_from(row.attempts)
        .map_err(|_| SkippedDeadJob::new(id.clone(), DeadJobSkip::NegativeAttempts))?;
    let requeue_count = next_count_value(row.requeue_count.as_deref())
        .map_err(|()| SkippedDeadJob::new(id.clone(), DeadJobSkip::InvalidRequeueCount))?;
    let last_error = row
        .last_error
        .map(|error| truncate_utf8(&error, MAX_LAST_ERROR_BYTES));
    Ok(DeadJob {
        id,
        job_type: row.job_type,
        reason: dead_reason_from_metadata(row.dead_reason.as_deref()),
        attempts,
        last_error,
        requeue_count,
        dead_at: row.dead_at,
    })
}

/// Report the rows one listing could not represent.
///
/// The summary line makes the size of the gap visible even when the per-row
/// lines are sampled away, and each row names only its identifier and a
/// bounded reason, so a malformed row cannot inflate a log line.
pub(super) fn report_skipped_rows(skipped: &[SkippedDeadJob]) {
    if skipped.is_empty() {
        return;
    }
    tracing::warn!(
        skipped = skipped.len(),
        "dead-letter listing omitted rows it could not represent"
    );
    for row in skipped {
        tracing::warn!(
            job_id = %row.id.as_str(),
            reason = row.cause.as_label(),
            "omitted dead-letter row"
        );
    }
}

/// Map the stored dead-reason label, treating absent or unknown values as
/// unclassified so an older row remains listable.
pub(super) fn dead_reason_from_metadata(value: Option<&str>) -> DeadReason {
    match value {
        Some("fatal") => DeadReason::Fatal,
        Some("exhausted") => DeadReason::Exhausted,
        _ => DeadReason::Unclassified,
    }
}

/// The requeue counter a restored row should carry: one more than it had,
/// saturating rather than wrapping.
pub(super) fn next_requeue_count(value: Option<&str>) -> Result<u32, ()> {
    let current = next_count_value(value)?;
    Ok(current.saturating_add(1))
}

/// Read a stored counter, bounding an out-of-range value instead of failing so
/// a counter written by another tool still lists.
pub(super) fn next_count_value(value: Option<&str>) -> Result<u32, ()> {
    value
        .map(|value| {
            value
                .parse::<u64>()
                .map(|count| count.min(u64::from(u32::MAX)) as u32)
                .map_err(|_| ())
        })
        .unwrap_or(Ok(0))
}

/// Truncate to at most `max_bytes`, backing up to the nearest character
/// boundary so a multi-byte code point is never split.
pub(super) fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}
