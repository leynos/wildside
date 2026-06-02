//! Unit and property tests for `GenericApalisRouteQueue<P, Q>`, covering
//! enqueue, tracing, metrics, and serialization without live Apalis storage.

use super::*;
use crate::domain::ports::NoOpRouteQueueMetrics;
use crate::outbound::queue::test_helpers::{
    FailingQueueProvider, FakeQueueProvider, RecordingRouteQueueMetrics,
};
use futures::future::join_all;
use insta::assert_snapshot;
use proptest::prelude::*;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing_test::traced_test;

#[cfg(feature = "metrics")]
mod metrics;
mod properties;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct TestPlan {
    name: String,
}

fn no_op_metrics() -> Arc<NoOpRouteQueueMetrics> {
    Arc::new(NoOpRouteQueueMetrics)
}
type TestQueue = GenericApalisRouteQueue<TestPlan, FakeQueueProvider>;
type EnqueueHandle = JoinHandle<Result<(), JobDispatchError>>;

fn spawn_enqueues(queue: Arc<TestQueue>, count: usize) -> Vec<EnqueueHandle> {
    (0..count)
        .map(|index| {
            let queue = Arc::clone(&queue);
            tokio::spawn(async move { queue.enqueue(&plan(index)).await })
        })
        .collect()
}
fn plan(index: usize) -> TestPlan {
    TestPlan {
        name: format!("plan-{index}"),
    }
}
async fn assert_all_enqueues_succeed(handles: Vec<EnqueueHandle>) {
    for result in join_all(handles).await {
        assert!(
            matches!(result, Ok(Ok(()))),
            "concurrent enqueue task should succeed: {result:?}"
        );
    }
}
async fn assert_plan_round_trips(plan: TestPlan) {
    let fake_provider = FakeQueueProvider::new();
    let queue: GenericApalisRouteQueue<TestPlan, _> =
        GenericApalisRouteQueue::new(fake_provider.clone(), no_op_metrics());

    let enqueue_result = queue.enqueue(&plan).await;
    assert!(
        enqueue_result.is_ok(),
        "enqueue should succeed with fake provider: {enqueue_result:?}"
    );

    let pushed_jobs = match fake_provider.pushed_jobs() {
        Ok(pushed_jobs) => pushed_jobs,
        Err(error) => panic!("should be able to access pushed jobs: {error}"),
    };
    assert_eq!(pushed_jobs.len(), 1, "exactly one job should be pushed");
    let deserialized: TestPlan = match serde_json::from_value(pushed_jobs[0].clone()) {
        Ok(plan) => plan,
        Err(error) => panic!("pushed payload should be valid JSON: {error}"),
    };
    assert_eq!(deserialized, plan, "deserialized plan should match");
}
async fn assert_failed_serialization_pushes_no_jobs(message: String) {
    let fake_provider = FakeQueueProvider::new();
    let queue: GenericApalisRouteQueue<FailingSerializePlan, _> =
        GenericApalisRouteQueue::new(fake_provider.clone(), no_op_metrics());

    let result = queue.enqueue(&FailingSerializePlan { message }).await;
    assert!(
        result.is_err(),
        "serialization failure should reject enqueue"
    );

    let pushed_jobs = match fake_provider.pushed_jobs() {
        Ok(pushed_jobs) => pushed_jobs,
        Err(error) => panic!("should be able to access pushed jobs: {error}"),
    };
    assert_eq!(
        pushed_jobs.len(),
        0,
        "no jobs should be pushed when serialization fails"
    );
}
fn block_on_property<F>(future: F)
where
    F: Future<Output = ()>,
{
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => panic!("test runtime should start: {error}"),
    };
    runtime.block_on(future);
}
proptest! {
    #[test]
    fn apalis_queue_round_trips_arbitrary_plan_names(name in "\\PC*") {
        block_on_property(assert_plan_round_trips(TestPlan { name }));
    }

    #[test]
    fn apalis_queue_failed_serialization_pushes_no_jobs(message in "\\PC*") {
        block_on_property(assert_failed_serialization_pushes_no_jobs(message));
    }
}

#[tokio::test]
async fn apalis_queue_enqueue_round_trips() {
    let fake_provider = FakeQueueProvider::new();
    let queue: GenericApalisRouteQueue<TestPlan, _> =
        GenericApalisRouteQueue::new(fake_provider.clone(), no_op_metrics());

    let plan = TestPlan {
        name: "test-plan".to_string(),
    };

    let result = queue.enqueue(&plan).await;
    assert!(result.is_ok(), "enqueue should succeed with fake provider");
    let pushed_jobs = fake_provider
        .pushed_jobs()
        .expect("should be able to access pushed jobs");
    assert_eq!(pushed_jobs.len(), 1, "exactly one job should be pushed");
    let deserialized: TestPlan = serde_json::from_value(pushed_jobs[0].clone())
        .expect("pushed payload should be valid JSON");
    assert_eq!(
        deserialized, plan,
        "deserialized plan should match original"
    );
}

#[tokio::test]
async fn apalis_queue_maps_provider_error_to_unavailable() {
    let failing_provider = FailingQueueProvider::new("simulated queue failure".to_string());
    let queue: GenericApalisRouteQueue<TestPlan, _> =
        GenericApalisRouteQueue::new(failing_provider, no_op_metrics());

    let plan = TestPlan {
        name: "test-plan".to_string(),
    };

    let result = queue.enqueue(&plan).await;
    assert!(
        result.is_err(),
        "enqueue should fail when provider returns error"
    );

    match result.expect_err("expected error but call succeeded") {
        error @ JobDispatchError::Unavailable { .. } => assert_snapshot!(
            error.to_string(),
            @"route queue is unavailable: simulated queue failure"
        ),
        JobDispatchError::Rejected { .. } => {
            panic!("expected Unavailable error, got Rejected");
        }
    }
}

#[tokio::test]
async fn apalis_queue_enqueues_multiple_plans() {
    let fake_provider = FakeQueueProvider::new();
    let queue: GenericApalisRouteQueue<TestPlan, _> =
        GenericApalisRouteQueue::new(fake_provider.clone(), no_op_metrics());

    let plan1 = TestPlan {
        name: "plan-1".to_string(),
    };
    let plan2 = TestPlan {
        name: "plan-2".to_string(),
    };

    queue
        .enqueue(&plan1)
        .await
        .expect("first enqueue should succeed");
    queue
        .enqueue(&plan2)
        .await
        .expect("second enqueue should succeed");

    let pushed_jobs = fake_provider
        .pushed_jobs()
        .expect("should be able to access pushed jobs");
    assert_eq!(pushed_jobs.len(), 2, "both jobs should be pushed");
    let deserialized1: TestPlan =
        serde_json::from_value(pushed_jobs[0].clone()).expect("first payload should be valid JSON");
    let deserialized2: TestPlan = serde_json::from_value(pushed_jobs[1].clone())
        .expect("second payload should be valid JSON");

    assert_eq!(deserialized1, plan1, "first plan should match");
    assert_eq!(deserialized2, plan2, "second plan should match");
}
#[tokio::test]
async fn concurrent_enqueue_pushes_all_jobs() {
    let fake_provider = Arc::new(FakeQueueProvider::new());
    let metrics = RecordingRouteQueueMetrics::default();
    let queue = Arc::new(TestQueue::new(
        fake_provider.as_ref().clone(),
        Arc::new(metrics.clone()),
    ));
    assert_all_enqueues_succeed(spawn_enqueues(queue, 8)).await;
    let pushed_jobs = fake_provider.pushed_jobs().expect("pushed jobs");
    assert_eq!(pushed_jobs.len(), 8, "all concurrent jobs should be pushed");
    let observations = metrics.observations().expect("metrics observations");
    assert_eq!(
        observations.len(),
        8,
        "all concurrent jobs should be observed"
    );
    assert!(observations.iter().all(|(outcome, latency)| {
        *outcome == RouteQueueOutcome::Success && *latency >= Duration::ZERO
    }));
}
#[derive(Debug, Clone)]
struct FailingSerializePlan {
    message: String,
}

impl Serialize for FailingSerializePlan {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Err(serde::ser::Error::custom(&self.message))
    }
}

#[tokio::test]
async fn apalis_queue_maps_serialization_failure_to_rejected() {
    let fake_provider = FakeQueueProvider::new();
    let queue: GenericApalisRouteQueue<FailingSerializePlan, _> =
        GenericApalisRouteQueue::new(fake_provider.clone(), no_op_metrics());

    let plan = FailingSerializePlan {
        message: "simulated serialization failure".to_string(),
    };

    let result = queue.enqueue(&plan).await;
    assert!(
        result.is_err(),
        "enqueue should fail when serialization fails"
    );

    match result.expect_err("expected error but call succeeded") {
        error @ JobDispatchError::Rejected { .. } => assert_snapshot!(
            error.to_string(),
            @"route job was rejected: Failed to serialize plan: simulated serialization failure"
        ),
        JobDispatchError::Unavailable { .. } => {
            panic!("expected Rejected error for serialization failure, got Unavailable");
        }
    }

    let pushed_jobs = fake_provider
        .pushed_jobs()
        .expect("should be able to access pushed jobs");
    assert_eq!(
        pushed_jobs.len(),
        0,
        "no jobs should be pushed when serialization fails"
    );
}
#[traced_test]
#[tokio::test]
async fn apalis_queue_success_logs_enqueue_without_alert() {
    let fake_provider = FakeQueueProvider::new();
    let queue: GenericApalisRouteQueue<TestPlan, _> =
        GenericApalisRouteQueue::new(fake_provider, no_op_metrics());
    let plan = TestPlan {
        name: "test-plan".to_string(),
    };

    queue
        .enqueue(&plan)
        .await
        .expect("enqueue should succeed with fake provider");

    assert!(
        logs_contain("enqueue"),
        "success logs should mention enqueue"
    );
    assert!(
        !logs_contain("warn") && !logs_contain("WARN"),
        "successful enqueue should not emit warning logs"
    );
}
#[traced_test]
#[tokio::test]
async fn apalis_queue_provider_failure_logs_level() {
    let failing_provider = FailingQueueProvider::new("simulated queue failure".to_string());
    let queue: GenericApalisRouteQueue<TestPlan, _> =
        GenericApalisRouteQueue::new(failing_provider, no_op_metrics());
    let plan = TestPlan {
        name: "test-plan".to_string(),
    };

    let result = queue.enqueue(&plan).await;

    assert!(
        result.is_err(),
        "enqueue should fail when provider returns error"
    );
    assert!(
        logs_contain("WARN"),
        "provider failure should emit a warning"
    );
    assert!(
        logs_contain("simulated queue failure"),
        "warning should include provider failure text"
    );
}
#[traced_test]
#[tokio::test]
async fn apalis_queue_serialization_failure_logs_level() {
    let fake_provider = FakeQueueProvider::new();
    let queue: GenericApalisRouteQueue<FailingSerializePlan, _> =
        GenericApalisRouteQueue::new(fake_provider, no_op_metrics());
    let plan = FailingSerializePlan {
        message: "simulated serialization failure".to_string(),
    };

    let result = queue.enqueue(&plan).await;

    assert!(
        result.is_err(),
        "enqueue should fail when serialization fails"
    );
    assert!(
        logs_contain("WARN"),
        "serialization failure should emit a warning"
    );
    assert!(
        logs_contain("serialization") || logs_contain("serialize"),
        "warning should mention serialization"
    );
}
