//! Mapping and dead-row predicate tests for the PostgreSQL adapter.

use super::row_decoding::{
    dead_job_from_row, dead_reason_from_metadata, next_count_value, next_requeue_count, skip_label,
    truncate_utf8,
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

#[test]
fn requeue_counter_starts_at_one_and_saturates_at_u32_max() {
    assert_eq!(next_requeue_count(None), Ok(1));
    assert_eq!(next_requeue_count(Some("0")), Ok(1));
    assert_eq!(next_requeue_count(Some("7")), Ok(8));
    assert_eq!(next_requeue_count(Some("4294967295")), Ok(u32::MAX));
    assert_eq!(next_requeue_count(Some("not-a-count")), Err(()));
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
