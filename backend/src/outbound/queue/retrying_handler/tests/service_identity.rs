//! Identity tests for the retry wrapper's persistence precondition.
//!
//! The wrapper may only persist a failure outcome while it can prove the
//! current worker owns the row. A delivery without a task identifier, without
//! a worker context, without `lock_by`, or with a worker that disagrees with
//! `lock_by` must record the failure metric and write nothing.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use apalis_core::error::BoxDynError;
use apalis_core::layers::Service;
use apalis_postgres::PgTask;
use serde_json::{Value, json};

use super::doubles::{
    RecordingMetrics, RecordingPersistence, TestJob, WORKER, WriteOutcome, service,
    task_with_identity, task_without_id,
};
use crate::domain::jobs::retry::{JobErrorKind, JobFailure};
use crate::domain::ports::RouteRetryOutcome;

/// One delivery through the wrapper, with every effect recorded.
struct Harness {
    persistence: Arc<RecordingPersistence>,
    metrics: Arc<RecordingMetrics>,
    runs: Arc<AtomicUsize>,
}

impl Harness {
    fn new() -> Self {
        Self {
            persistence: Arc::new(RecordingPersistence::new(
                WriteOutcome::Updated,
                WriteOutcome::Updated,
            )),
            metrics: Arc::new(RecordingMetrics::default()),
            runs: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Deliver `payload` once through a task with the chosen identities.
    ///
    /// The handler always fails retryably, so the wrapper's persistence
    /// precondition is the only thing that decides whether a write happens.
    async fn deliver(
        &self,
        payload: Value,
        worker: Option<&'static str>,
        lock_by: Option<&'static str>,
    ) -> Result<(), BoxDynError> {
        self.deliver_task(task_with_identity(payload, 1, worker, lock_by))
            .await
    }

    /// Deliver a task that carries no identifier.
    async fn deliver_without_id(
        &self,
        payload: Value,
        worker: Option<&'static str>,
        lock_by: Option<&'static str>,
    ) -> Result<(), BoxDynError> {
        self.deliver_task(task_without_id(payload, 1, worker, lock_by))
            .await
    }

    async fn deliver_task(&self, task: PgTask<Value>) -> Result<(), BoxDynError> {
        let runs = Arc::clone(&self.runs);
        let handler = move |_job: TestJob, _attempt| {
            runs.fetch_add(1, Ordering::SeqCst);
            async { Err(JobFailure::new(JobErrorKind::Retryable, "probe failure")) }
        };
        let mut service = service(
            Arc::clone(&self.persistence),
            Arc::clone(&self.metrics),
            handler,
        );
        service.call(task).await
    }

    /// Assert the delivery failed, ran its handler, and wrote nothing.
    fn assert_refused(&self) {
        assert!(
            self.persistence.calls().is_empty(),
            "the wrapper must not persist when ownership is unproven"
        );
        assert_eq!(
            self.metrics.retries(),
            vec![RouteRetryOutcome::ScheduleFailure],
            "an unpersisted retry is a scheduling failure"
        );
        assert!(self.metrics.dead_letters().is_empty());
        assert_eq!(self.runs.load(Ordering::SeqCst), 1, "the handler must run");
    }
}

#[tokio::test]
async fn missing_task_id_writes_nothing() {
    let harness = Harness::new();

    let result = harness
        .deliver_without_id(
            json!({"v": "v1", "probeId": "probe"}),
            Some(WORKER),
            Some(WORKER),
        )
        .await;

    assert!(result.is_err(), "the delivery stays failed");
    harness.assert_refused();
}

#[tokio::test]
async fn missing_worker_context_writes_nothing() {
    let harness = Harness::new();

    let result = harness
        .deliver(json!({"v": "v1", "probeId": "probe"}), None, Some(WORKER))
        .await;

    assert!(result.is_err(), "the delivery stays failed");
    harness.assert_refused();
}

#[tokio::test]
async fn missing_lock_by_writes_nothing() {
    let harness = Harness::new();

    let result = harness
        .deliver(json!({"v": "v1", "probeId": "probe"}), Some(WORKER), None)
        .await;

    assert!(result.is_err(), "the delivery stays failed");
    harness.assert_refused();
}

#[tokio::test]
async fn mismatched_worker_and_lock_by_writes_nothing() {
    let harness = Harness::new();

    let result = harness
        .deliver(
            json!({"v": "v1", "probeId": "probe"}),
            Some(WORKER),
            Some("another-worker"),
        )
        .await;

    assert!(result.is_err(), "the delivery stays failed");
    harness.assert_refused();
}

#[tokio::test]
async fn matching_worker_and_lock_by_persists_the_retry() {
    let harness = Harness::new();

    let result = harness
        .deliver(
            json!({"v": "v1", "probeId": "probe"}),
            Some(WORKER),
            Some(WORKER),
        )
        .await;

    assert!(result.is_err(), "the delivery stays failed");
    assert_eq!(harness.persistence.calls().len(), 1);
    assert_eq!(
        harness.metrics.retries(),
        vec![RouteRetryOutcome::Scheduled]
    );
}
