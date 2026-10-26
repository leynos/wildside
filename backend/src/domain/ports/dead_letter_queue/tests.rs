//! Contract tests for dead-job identifiers and port errors.

use super::{DeadJobId, DeadLetterError};

#[test]
fn dead_job_id_keeps_the_storage_identifier_opaque() {
    let id = DeadJobId::new("opaque-task-id");

    assert_eq!(id.as_str(), "opaque-task-id");
    assert_eq!(id, DeadJobId::new("opaque-task-id"));
}

#[test]
fn not_dead_error_explains_a_failed_guard() {
    let error = DeadLetterError::not_dead("row is pending");

    assert_eq!(error.to_string(), "dead job is not dead: row is pending");
}

#[test]
fn unknown_queue_error_explains_the_missing_budget() {
    let error = DeadLetterError::unknown_queue("missing-route-queue");

    assert_eq!(
        error.to_string(),
        "dead job queue has no retry budget: missing-route-queue"
    );
}

#[test]
fn unavailable_error_preserves_storage_context() {
    let error = DeadLetterError::unavailable("connection lost");

    assert_eq!(
        error.to_string(),
        "dead-letter storage is unavailable: connection lost"
    );
}
