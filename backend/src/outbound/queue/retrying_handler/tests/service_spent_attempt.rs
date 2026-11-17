//! Delivery-charge tests for the wrapper's pre-handler attempt guard.
//!
//! Apalis counts a row's `attempts` as the deliveries it has already received
//! and increments the in-memory attempt on the first poll, so a legal delivery
//! carries an attempt at or below the stored `max_attempts`. Orphan recovery
//! breaks that relation: it returns a retired row to `Pending` and increments
//! `attempts`, while the fetch predicate accepts `Pending` rows without
//! consulting the cap. These tests pin both sides of the guard — a delivery
//! past the cap never reaches handler code, and the legal final delivery
//! still does.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use apalis_core::error::BoxDynError;
use apalis_core::layers::Service;
use rstest::rstest;
use serde_json::json;

use super::doubles::{
    DeliveryIdentity, RecordingMetrics, RecordingPersistence, WORKER, WriteOutcome, service,
    task_with_budget,
};

/// Deliver once through the wrapper with a chosen attempt and stored cap.
async fn deliver(
    attempt: usize,
    max_attempts: i32,
    runs: &Arc<AtomicUsize>,
) -> Result<(), BoxDynError> {
    let persistence = Arc::new(RecordingPersistence::new(
        WriteOutcome::Updated,
        WriteOutcome::Updated,
    ));
    let metrics = Arc::new(RecordingMetrics::default());
    let counted = Arc::clone(runs);
    let handler = move |_job, _attempt| {
        counted.fetch_add(1, Ordering::SeqCst);
        async { Ok(()) }
    };
    let mut wrapper = service(persistence, metrics, handler);
    let identity = DeliveryIdentity::new(Some(WORKER), Some(WORKER));
    wrapper
        .call(task_with_budget(
            json!({"v": "v1", "probeId": "probe"}),
            attempt,
            max_attempts,
            identity,
        ))
        .await
}

/// Scenario: orphan recovery returned a row whose cap its attempt now exceeds.
///
/// Invariant: the wrapper refuses the delivery without invoking the handler,
/// so a job already classified as terminal is not run again.
#[rstest]
#[case::recovered_past_the_cap(6, 5)]
#[case::cap_of_zero_admits_nothing(1, 0)]
#[tokio::test]
async fn delivery_past_the_stored_cap_never_reaches_the_handler(
    #[case] attempt: usize,
    #[case] max_attempts: i32,
) {
    let runs = Arc::new(AtomicUsize::new(0));

    let result = deliver(attempt, max_attempts, &runs).await;

    assert!(
        result.is_err(),
        "a refused delivery stays failed for the acknowledgement"
    );
    assert_eq!(
        runs.load(Ordering::SeqCst),
        0,
        "handler code must not run for a spent attempt cap"
    );
}

/// Scenario: the delivery that spends the last permitted attempt.
///
/// Invariant: the guard admits it, because the cap bounds how many deliveries
/// may run rather than refusing the one that reaches it.
#[rstest]
#[case::first_of_one(1, 1)]
#[case::last_of_five(5, 5)]
#[tokio::test]
async fn delivery_at_the_stored_cap_still_reaches_the_handler(
    #[case] attempt: usize,
    #[case] max_attempts: i32,
) {
    let runs = Arc::new(AtomicUsize::new(0));

    let result = deliver(attempt, max_attempts, &runs).await;

    assert!(result.is_ok(), "the permitted delivery must complete");
    assert_eq!(
        runs.load(Ordering::SeqCst),
        1,
        "the permitted delivery must reach the handler"
    );
}
