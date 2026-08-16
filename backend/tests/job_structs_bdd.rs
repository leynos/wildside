//! Behaviour tests for domain job structs and queue payload serialization.

use std::cell::RefCell;
use std::fmt;
use std::sync::Arc;

use backend::domain::jobs::{BoundingBox, EnrichmentJob, EnrichmentJobParams, GenerateRouteJob};
use backend::domain::ports::{
    EnrichmentRequest, JobDispatchError, NoOpRouteQueueMetrics, RouteQueue, RouteSubmissionRequest,
};
use backend::domain::{IdempotencyKey, UserId};
use backend::outbound::queue::test_helpers::FakeQueueProvider;
use backend::outbound::queue::{GenericApalisRouteQueue, StubRouteQueue};
use chrono::{DateTime, Utc};
use insta::assert_json_snapshot;
use pretty_assertions::assert_eq;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use serde::ser::{Serialize, Serializer};
use serde_json::{Value, json};
use tokio::runtime::Runtime;
use uuid::Uuid;

struct JobStructWorld {
    runtime: Runtime,
    route_submission: RefCell<Option<RouteSubmissionRequest>>,
    generated_route_job: RefCell<Option<GenerateRouteJob>>,
    stub_enqueue_result: RefCell<Option<Result<(), JobDispatchError>>>,
    enrichment_job: RefCell<Option<EnrichmentJob>>,
    fake_payloads: RefCell<Vec<Value>>,
    queue_error: RefCell<Option<JobDispatchError>>,
    enrichment_request: RefCell<Option<EnrichmentRequest>>,
}

impl JobStructWorld {
    fn new() -> Self {
        Self {
            runtime: Runtime::new().expect("test runtime should start"),
            route_submission: RefCell::new(None),
            generated_route_job: RefCell::new(None),
            stub_enqueue_result: RefCell::new(None),
            enrichment_job: RefCell::new(None),
            fake_payloads: RefCell::new(Vec::new()),
            queue_error: RefCell::new(None),
            enrichment_request: RefCell::new(None),
        }
    }
}

#[fixture]
fn world() -> JobStructWorld {
    JobStructWorld::new()
}

#[derive(Debug)]
struct FailingSerializePlan;

impl Serialize for FailingSerializePlan {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Err(serde::ser::Error::custom(
            "intentional serialization failure",
        ))
    }
}

impl fmt::Display for FailingSerializePlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("failing-serialize-plan")
    }
}

fn request_id() -> Uuid {
    Uuid::from_bytes([0x11; 16])
}

fn job_id() -> Uuid {
    Uuid::from_bytes([0x44; 16])
}

fn user_id() -> UserId {
    UserId::from_uuid(Uuid::from_bytes([0x22; 16]))
}

fn idempotency_key() -> IdempotencyKey {
    IdempotencyKey::from_uuid(Uuid::from_bytes([0x33; 16]))
}

fn enrichment_idempotency_key() -> IdempotencyKey {
    IdempotencyKey::from_uuid(Uuid::from_bytes([0x55; 16]))
}

fn route_enqueued_at() -> DateTime<Utc> {
    parse_timestamp("2026-06-14T12:00:00Z")
}

fn enrichment_enqueued_at() -> DateTime<Utc> {
    parse_timestamp("2026-06-14T12:30:00Z")
}

fn parse_timestamp(value: &'static str) -> DateTime<Utc> {
    match DateTime::parse_from_rfc3339(value) {
        Ok(timestamp) => timestamp.with_timezone(&Utc),
        Err(error) => panic!("static timestamp should parse: {error}"),
    }
}

fn valid_route_submission() -> RouteSubmissionRequest {
    RouteSubmissionRequest {
        idempotency_key: Some(idempotency_key()),
        user_id: user_id(),
        payload: serde_json::from_value(json!({
            "origin": { "lat": 51.5074, "lng": -0.1278 },
            "destination": { "lat": 51.5014, "lng": -0.1419 },
            "preferences": { "mode": "walking" }
        }))
        .expect("route submission fixture should be valid"),
    }
}

fn valid_bounding_box() -> BoundingBox {
    match BoundingBox::new(-0.20, 51.40, 0.10, 51.60) {
        Ok(bounding_box) => bounding_box,
        Err(error) => panic!("static bounding box should be valid: {error}"),
    }
}

fn valid_enrichment_job() -> EnrichmentJob {
    match EnrichmentJob::v1(EnrichmentJobParams {
        job_id: job_id(),
        idempotency_key: Some(enrichment_idempotency_key()),
        bounding_box: valid_bounding_box(),
        tags: vec!["tourism".to_owned(), "amenity".to_owned()],
        enqueued_at: enrichment_enqueued_at(),
    }) {
        Ok(job) => job,
        Err(error) => panic!("static enrichment job should be valid: {error}"),
    }
}

#[given("a valid route submission")]
fn a_valid_route_submission(world: &JobStructWorld) {
    *world.route_submission.borrow_mut() = Some(valid_route_submission());
}

#[given("a valid enrichment job")]
fn a_valid_enrichment_job(world: &JobStructWorld) {
    *world.enrichment_job.borrow_mut() = Some(valid_enrichment_job());
}

#[given("a plan that fails serialization")]
fn a_plan_that_fails_serialization(world: &JobStructWorld) {
    *world.queue_error.borrow_mut() = None;
}

#[when("I build and enqueue a generate-route job through the stub queue")]
fn i_build_and_enqueue_a_generate_route_job_through_the_stub_queue(world: &JobStructWorld) {
    let submission = world.route_submission.borrow();
    let submission = submission
        .as_ref()
        .expect("route submission should be configured");
    let job = GenerateRouteJob::try_from_submission(submission, request_id(), route_enqueued_at());
    let queue: StubRouteQueue<GenerateRouteJob> = StubRouteQueue::new();
    let enqueue_result = world.runtime.block_on(async { queue.enqueue(&job).await });

    *world.generated_route_job.borrow_mut() = Some(job);
    *world.stub_enqueue_result.borrow_mut() = Some(enqueue_result);
}

#[when("I enqueue the enrichment job through the fake Apalis queue")]
fn i_enqueue_the_enrichment_job_through_the_fake_apalis_queue(world: &JobStructWorld) {
    let enrichment_job = world.enrichment_job.borrow();
    let job = enrichment_job
        .as_ref()
        .expect("enrichment job should be configured");
    let provider = FakeQueueProvider::new();
    let queue: GenericApalisRouteQueue<EnrichmentJob, _> =
        GenericApalisRouteQueue::new(provider.clone(), Arc::new(NoOpRouteQueueMetrics));
    if let Err(error) = world.runtime.block_on(async { queue.enqueue(job).await }) {
        panic!("enrichment job should enqueue through the fake Apalis queue: {error}");
    }
    // Release the borrow before writing back into another world cell.
    drop(enrichment_job);
    let payloads = match provider.pushed_jobs() {
        Ok(payloads) => payloads,
        Err(error) => panic!("fake provider payloads should be readable: {error}"),
    };
    *world.fake_payloads.borrow_mut() = payloads;
}

#[when("I enqueue the failing plan through the fake Apalis queue")]
fn i_enqueue_the_failing_plan_through_the_fake_apalis_queue(world: &JobStructWorld) {
    let provider = FakeQueueProvider::new();
    let queue: GenericApalisRouteQueue<FailingSerializePlan, _> =
        GenericApalisRouteQueue::new(provider, Arc::new(NoOpRouteQueueMetrics));
    let result = world
        .runtime
        .block_on(async { queue.enqueue(&FailingSerializePlan).await });

    *world.queue_error.borrow_mut() = result.err();
}

#[when("I convert the enrichment job to an enrichment request")]
fn i_convert_the_enrichment_job_to_an_enrichment_request(world: &JobStructWorld) {
    let enrichment_job = world.enrichment_job.borrow();
    let request = enrichment_job
        .as_ref()
        .expect("enrichment job should be configured")
        .to_enrichment_request();
    // Release the borrow before writing back into another world cell.
    drop(enrichment_job);

    *world.enrichment_request.borrow_mut() = Some(request);
}

#[then("the stub enqueue succeeds")]
fn the_stub_enqueue_succeeds(world: &JobStructWorld) {
    let result = world.stub_enqueue_result.borrow();
    let result = result.as_ref().expect("stub enqueue should have run");

    assert!(result.is_ok(), "stub enqueue should succeed: {result:?}");
}

#[then("the fake queue records the enrichment JSON payload")]
fn the_fake_queue_records_the_enrichment_json_payload(world: &JobStructWorld) {
    let payloads = world.fake_payloads.borrow();

    assert_eq!(payloads.len(), 1, "fake queue should record one payload");
    assert_json_snapshot!("job_structs_bdd_enrichment_queue_payload", payloads[0]);
}

#[then("the queue returns a rejected dispatch error")]
fn the_queue_returns_a_rejected_dispatch_error(world: &JobStructWorld) {
    let error = world.queue_error.borrow();
    let error = error
        .as_ref()
        .expect("failing plan should produce a dispatch error");

    assert!(
        matches!(error, JobDispatchError::Rejected { .. }),
        "serialization failure should map to rejected error: {error:?}"
    );
    assert!(
        error.to_string().contains("Failed to serialize plan"),
        "error should include serialization context: {error}"
    );
}

#[then("the enrichment request preserves the job fields")]
fn the_enrichment_request_preserves_the_job_fields(world: &JobStructWorld) {
    let request = world.enrichment_request.borrow();
    let request = request
        .as_ref()
        .expect("enrichment conversion should have run");

    assert_eq!(request.job_id, job_id());
    assert_eq!(request.bounding_box, valid_bounding_box());
    assert_eq!(
        request.tags,
        vec!["amenity".to_owned(), "tourism".to_owned()]
    );
}

#[scenario(
    path = "tests/features/job_structs.feature",
    name = "Build a generate-route job from a submission and enqueue via stub"
)]
fn build_generate_route_job_from_submission_and_enqueue_via_stub(world: JobStructWorld) {
    let result = world.stub_enqueue_result.borrow();
    assert!(
        matches!(result.as_ref(), Some(Ok(()))),
        "scenario should end with a successful stub enqueue"
    );
}

#[scenario(
    path = "tests/features/job_structs.feature",
    name = "Build an enrichment job and observe its queue payload"
)]
fn build_enrichment_job_and_observe_queue_payload(world: JobStructWorld) {
    assert_eq!(
        world.fake_payloads.borrow().len(),
        1,
        "scenario should record one queue payload"
    );
}

#[scenario(
    path = "tests/features/job_structs.feature",
    name = "Surface a serialization rejection"
)]
fn surface_serialization_rejection(world: JobStructWorld) {
    let error = world.queue_error.borrow();
    assert!(
        matches!(error.as_ref(), Some(JobDispatchError::Rejected { .. })),
        "scenario should end with a rejected dispatch error"
    );
}

#[scenario(
    path = "tests/features/job_structs.feature",
    name = "Convert an enrichment job to an enrichment request"
)]
fn convert_enrichment_job_to_enrichment_request(world: JobStructWorld) {
    assert!(
        world.enrichment_request.borrow().is_some(),
        "scenario should create an enrichment request"
    );
}
