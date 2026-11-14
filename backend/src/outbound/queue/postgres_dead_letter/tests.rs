//! Mapping and dead-row predicate tests for the PostgreSQL adapter.

use super::{
    dead_reason_from_metadata, is_dead_row, next_count_value, next_requeue_count, truncate_utf8,
};
use crate::domain::ports::DeadReason;

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
