//! Mapping and dead-row predicate tests for the PostgreSQL adapter.

use super::row_decoding::{
    RequeueCountError, dead_job_from_row, dead_reason_from_metadata, next_count_value,
    next_requeue_count, skip_label, truncate_utf8,
};
use super::{DeadJobRow, is_dead_row};
use crate::domain::ports::{DeadJobId, DeadJobSkip, DeadReason, SkippedDeadJob};

/// Build the storage row the conversion reads, filling unused columns with
/// values the case does not depend on.
fn stored_row(attempts: i32, requeue_count: Option<&str>) -> DeadJobRow {
    DeadJobRow {
        id: "row-1".to_owned(),
        job_type: "wildside.test-queue".to_owned(),
        attempts,
        dead_reason: Some("exhausted".to_owned()),
        requeue_count: requeue_count.map(str::to_owned),
        last_error: Some("probe failure".to_owned()),
        dead_at: None,
    }
}

#[rstest::rstest]
#[case::negative_attempts(stored_row(-1, None), DeadJobSkip::NegativeAttempts)]
#[case::invalid_requeue_count(stored_row(3, Some("not-a-count")), DeadJobSkip::InvalidRequeueCount)]
fn unrepresentable_rows_are_rejected_with_their_cause(
    #[case] row: DeadJobRow,
    #[case] cause: DeadJobSkip,
) {
    let rejected = dead_job_from_row(row).expect_err("the row cannot be represented");

    assert_eq!(
        rejected,
        SkippedDeadJob::new(DeadJobId::new("row-1"), cause)
    );
}

#[test]
fn representable_rows_map_every_listed_field() {
    let job = dead_job_from_row(stored_row(3, Some("2"))).expect("the row is representable");

    assert_eq!(job.id.as_str(), "row-1");
    assert_eq!(job.job_type, "wildside.test-queue");
    assert_eq!(job.reason, DeadReason::Exhausted);
    assert_eq!(job.attempts, 3);
    assert_eq!(job.requeue_count, 2);
    assert_eq!(job.last_error.as_deref(), Some("probe failure"));
    assert!(job.dead_at.is_none());
}

#[test]
fn skip_labels_stay_bounded_and_stable() {
    assert_eq!(
        skip_label(DeadJobSkip::NegativeAttempts),
        "negative-attempts",
        "a negative attempt count logs the negative-attempts reason"
    );
    assert_eq!(
        skip_label(DeadJobSkip::InvalidRequeueCount),
        "invalid-requeue-count",
        "an unparsable requeue count logs the invalid-requeue-count reason"
    );
}

#[test]
fn metadata_reason_maps_only_the_two_owned_labels() {
    assert_eq!(dead_reason_from_metadata(Some("fatal")), DeadReason::Fatal);
    assert_eq!(
        dead_reason_from_metadata(Some("exhausted")),
        DeadReason::Exhausted
    );
    assert_eq!(dead_reason_from_metadata(None), DeadReason::Unclassified);
    assert_eq!(
        dead_reason_from_metadata(Some("unknown")),
        DeadReason::Unclassified
    );
}

#[rstest::rstest]
#[case::killed("Killed", 1, Some(25), true)]
#[case::failed_at_limit("Failed", 3, Some(3), true)]
#[case::legacy_null_cap("Failed", 3, None, true)]
#[case::failed_below_limit("Failed", 2, Some(3), false)]
#[case::pending("Pending", 3, Some(3), false)]
#[case::running("Running", 3, Some(3), false)]
fn dead_predicate_matches_only_terminal_rows(
    #[case] status: &str,
    #[case] attempts: i32,
    #[case] max_attempts: Option<i32>,
    #[case] expected: bool,
) {
    assert_eq!(is_dead_row(status, attempts, max_attempts), expected);
}

/// Scenario: a requeue advances a stored counter.
///
/// Invariant: every accepted requeue writes a different value, because the
/// counter is the claim's delivery generation and a repeated generation would
/// let a delivery fetched before the requeue claim the restored row.
#[test]
fn requeue_counter_starts_at_one_and_advances() {
    assert_eq!(next_requeue_count(None), Ok(1));
    assert_eq!(
        next_requeue_count(Some("0")),
        Ok(1),
        "an empty counter advances to the first generation"
    );
    assert_eq!(
        next_requeue_count(Some("7")),
        Ok(8),
        "an ordinary counter advances by one"
    );
    assert_eq!(
        next_requeue_count(Some("4294967294")),
        Ok(u32::MAX),
        "the last counter below the ceiling still advances"
    );
}

/// Scenario: the stored counter already holds its maximum.
///
/// Invariant: the requeue is refused rather than writing the value the row
/// already carries, because a saturated counter cannot name a new generation.
#[test]
fn a_saturated_requeue_counter_is_refused() {
    assert_eq!(
        next_requeue_count(Some("4294967295")),
        Err(RequeueCountError::Exhausted),
        "a counter at its maximum cannot advance, so the requeue must be refused"
    );
    assert_eq!(
        next_requeue_count(Some("4294967296")),
        Err(RequeueCountError::Exhausted),
        "an out-of-range stored counter is bounded to the maximum and refused"
    );
    assert_eq!(
        next_requeue_count(Some("not-a-count")),
        Err(RequeueCountError::NotNumeric),
        "a non-numeric counter is a different fault from an exhausted one"
    );
}

#[test]
fn out_of_range_stored_counter_is_bounded_for_listing() {
    assert_eq!(next_count_value(Some("4294967296")), Ok(u32::MAX));
    assert_eq!(next_count_value(Some("-1")), Err(()));
}

#[test]
fn last_error_truncation_preserves_utf8_boundaries() {
    let value = format!("{}🧭", "x".repeat(1023));
    let truncated = truncate_utf8(&value, 1024);

    assert_eq!(truncated.len(), 1023);
    assert!(truncated.is_char_boundary(truncated.len()));
}
