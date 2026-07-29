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

macro_rules! assert_enqueue_observation {
    ($observations:expr, $expected:expr) => {{
        assert_eq!($observations.len(), 1, "one enqueue observation");
        assert_eq!($observations[0].0, $expected, "outcome maps to path");
    }};
}

macro_rules! assert_successful_enqueue_observations {
    ($observations:expr, $expected_count:expr) => {{
        assert_eq!(
            $observations.len(),
            $expected_count,
            "one observation per job"
        );
        assert!(
            $observations
                .iter()
                .all(|(outcome, _)| *outcome == RouteQueueOutcome::Success),
            "every enqueue should report success: {:?}",
            $observations
        );
    }};
}

proptest! {
    #[test]
    fn metrics_observe_all_enqueue_paths(path in enqueue_path_strategy()) {
        let (observations, expected) = block_on_property(observe_enqueue_path(path))
            .expect("enqueue path should produce an observation");
        assert_enqueue_observation!(observations, expected);
    }

    #[test]
    fn concurrent_enqueue_observes_each_successful_job(job_count in 1_usize..=8) {
        let observations = block_on_property(observe_concurrent_enqueue_timing(job_count))
            .expect("concurrent enqueues should produce observations");
        assert_successful_enqueue_observations!(observations, job_count);
    }
}

fn enqueue_path_strategy() -> impl Strategy<Value = EnqueuePath> {
    prop_oneof![
        Just(EnqueuePath::Success),
        Just(EnqueuePath::ProviderFailure),
        Just(EnqueuePath::SerializationFailure),
    ]
}

/// Exercise one enqueue path and return its observations and expected outcome.
async fn observe_enqueue_path(
    path: EnqueuePath,
) -> Result<(Vec<(RouteQueueOutcome, Duration)>, RouteQueueOutcome), String> {
    let metrics = RecordingRouteQueueMetrics::default();
    let expected = match path {
        EnqueuePath::Success => enqueue_success(&metrics).await?,
        EnqueuePath::ProviderFailure => enqueue_provider_failure(&metrics).await?,
        EnqueuePath::SerializationFailure => enqueue_serialization_failure(&metrics).await?,
    };
    Ok((metrics.observations()?, expected))
}

/// Exercise a successful enqueue and return its expected metric outcome.
async fn enqueue_success(
    metrics: &RecordingRouteQueueMetrics,
) -> Result<RouteQueueOutcome, String> {
    let queue = GenericApalisRouteQueue::new(FakeQueueProvider::new(), Arc::new(metrics.clone()));
    queue
        .enqueue(&TestPlan { name: "ok".into() })
        .await
        .map_err(|error| format!("success-path enqueue failed: {error}"))?;
    Ok(RouteQueueOutcome::Success)
}

/// Exercise a provider failure and return its expected metric outcome.
async fn enqueue_provider_failure(
    metrics: &RecordingRouteQueueMetrics,
) -> Result<RouteQueueOutcome, String> {
    let provider = FailingQueueProvider::new("provider down".into());
    let queue = GenericApalisRouteQueue::new(provider, Arc::new(metrics.clone()));
    if queue
        .enqueue(&TestPlan {
            name: "fail".into(),
        })
        .await
        .is_ok()
    {
        return Err("provider-failure enqueue unexpectedly succeeded".to_string());
    }
    Ok(RouteQueueOutcome::Failure)
}

/// Exercise serialization failure and return its expected metric outcome.
async fn enqueue_serialization_failure(
    metrics: &RecordingRouteQueueMetrics,
) -> Result<RouteQueueOutcome, String> {
    let queue = GenericApalisRouteQueue::new(FakeQueueProvider::new(), Arc::new(metrics.clone()));
    let plan = FailingSerializePlan {
        message: "serialize down".into(),
    };
    if queue.enqueue(&plan).await.is_ok() {
        return Err("serialization-failure enqueue unexpectedly succeeded".to_string());
    }
    Ok(RouteQueueOutcome::Failure)
}

/// Return metric observations recorded for concurrent successful enqueues.
async fn observe_concurrent_enqueue_timing(
    job_count: usize,
) -> Result<Vec<(RouteQueueOutcome, Duration)>, String> {
    let provider = Arc::new(FakeQueueProvider::new());
    let metrics = RecordingRouteQueueMetrics::default();
    let queue = Arc::new(TestQueue::new(
        provider.as_ref().clone(),
        Arc::new(metrics.clone()),
    ));

    ensure_all_enqueues_succeed(spawn_enqueues(queue, job_count)).await?;

    metrics.observations()
}
