//! Property tests for route queue outcome and latency observations.
//!
//! These tests exercise invariants over success, provider failure, and
//! serialization failure paths without binding to a concrete metrics backend.

use super::*;

#[derive(Debug, Clone, Copy)]
enum EnqueuePath {
    Success,
    ProviderFailure,
    SerializationFailure,
}

proptest! {
    #[test]
    fn metrics_observe_all_enqueue_paths(path in enqueue_path_strategy()) {
        block_on_property(assert_enqueue_observes_expected_metrics(path));
    }

    #[test]
    fn concurrent_enqueue_observes_each_successful_job(job_count in 1_usize..=8) {
        block_on_property(assert_concurrent_enqueue_timing_invariants(job_count));
    }
}

fn enqueue_path_strategy() -> impl Strategy<Value = EnqueuePath> {
    prop_oneof![
        Just(EnqueuePath::Success),
        Just(EnqueuePath::ProviderFailure),
        Just(EnqueuePath::SerializationFailure),
    ]
}

async fn assert_enqueue_observes_expected_metrics(path: EnqueuePath) {
    let metrics = RecordingRouteQueueMetrics::default();
    let expected = match path {
        EnqueuePath::Success => enqueue_success(&metrics).await,
        EnqueuePath::ProviderFailure => enqueue_provider_failure(&metrics).await,
        EnqueuePath::SerializationFailure => enqueue_serialization_failure(&metrics).await,
    };
    let observations = metrics.observations().expect("metrics observations");
    assert_eq!(observations.len(), 1, "one enqueue observation");
    assert_eq!(observations[0].0, expected, "outcome maps to path");
    assert!(
        observations[0].1 >= Duration::ZERO,
        "latency cannot be negative"
    );
}

async fn enqueue_success(metrics: &RecordingRouteQueueMetrics) -> RouteQueueOutcome {
    let queue = GenericApalisRouteQueue::new(FakeQueueProvider::new(), Arc::new(metrics.clone()));
    queue
        .enqueue(&TestPlan { name: "ok".into() })
        .await
        .expect("enqueue");
    RouteQueueOutcome::Success
}

async fn enqueue_provider_failure(metrics: &RecordingRouteQueueMetrics) -> RouteQueueOutcome {
    let provider = FailingQueueProvider::new("provider down".into());
    let queue = GenericApalisRouteQueue::new(provider, Arc::new(metrics.clone()));
    assert!(
        queue
            .enqueue(&TestPlan {
                name: "fail".into(),
            })
            .await
            .is_err()
    );
    RouteQueueOutcome::Failure
}

async fn enqueue_serialization_failure(metrics: &RecordingRouteQueueMetrics) -> RouteQueueOutcome {
    let queue = GenericApalisRouteQueue::new(FakeQueueProvider::new(), Arc::new(metrics.clone()));
    let plan = FailingSerializePlan {
        message: "serialize down".into(),
    };
    assert!(queue.enqueue(&plan).await.is_err());
    RouteQueueOutcome::Failure
}

async fn assert_concurrent_enqueue_timing_invariants(job_count: usize) {
    let provider = Arc::new(FakeQueueProvider::new());
    let metrics = RecordingRouteQueueMetrics::default();
    let queue = Arc::new(TestQueue::new(
        provider.as_ref().clone(),
        Arc::new(metrics.clone()),
    ));

    assert_all_enqueues_succeed(spawn_enqueues(queue, job_count)).await;

    let observations = metrics.observations().expect("metrics observations");
    assert_eq!(observations.len(), job_count, "one observation per job");
    assert!(observations.iter().all(|(outcome, latency)| {
        *outcome == RouteQueueOutcome::Success && *latency >= Duration::ZERO
    }));
}
