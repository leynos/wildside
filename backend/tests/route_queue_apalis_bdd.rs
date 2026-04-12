//! BDD tests for Apalis-backed RouteQueue adapter against embedded PostgreSQL.
//!
//! These tests verify that the Apalis-backed `RouteQueue` adapter correctly
//! persists jobs to PostgreSQL storage and handles error conditions appropriately.
//! Tests use `pg-embedded-setup-unpriv` for isolated database instances.
//!
//! # Runtime Strategy
//!
//! This suite keeps synchronous steps and reuses a shared Tokio runtime in the
//! test context. This keeps queue operations deterministic and avoids recreating
//! a runtime for each step.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use backend::domain::ports::{JobDispatchError, RouteQueue};
use backend::outbound::queue::{ApalisPostgresProvider, GenericApalisRouteQueue, QueueProvider};
use pg_embedded_setup_unpriv::TemporaryDatabase;
use rstest::{fixture, rstest};
use rstest_bdd_macros::{given, scenario, then, when};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use tokio::runtime::Runtime;

mod support;

use support::atexit_cleanup::shared_cluster_handle;
use support::{handle_cluster_setup_failure, provision_template_database};

/// Simple test plan type for verifying queue behaviour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct TestPlan {
    name: String,
}

/// Test helper: Failing queue provider for simulating database unavailability.
#[derive(Clone)]
struct FailingProvider {
    error_message: String,
}

impl FailingProvider {
    fn new(error_message: String) -> Self {
        Self { error_message }
    }
}

#[async_trait]
impl QueueProvider for FailingProvider {
    async fn push_job(&self, _payload: Value) -> Result<(), JobDispatchError> {
        Err(JobDispatchError::unavailable(self.error_message.clone()))
    }
}

struct TestContext {
    /// Tokio runtime reused for all async operations in this test.
    runtime: Runtime,
    /// The queue adapter under test (or None if using invalid connection).
    /// Uses Arc<dyn RouteQueue<Plan = TestPlan>> to allow different provider implementations.
    queue: Option<Arc<dyn RouteQueue<Plan = TestPlan>>>,
    /// SQLx pool for verifying job persistence.
    pool: Option<PgPool>,
    /// Results of enqueue operations.
    enqueue_results: Vec<Result<(), JobDispatchError>>,
    /// Plans that were enqueued for later verification.
    enqueued_plans: Vec<TestPlan>,
    /// Temporary database handle (must outlive pool and queue).
    _database: TemporaryDatabase,
}

type SharedContext = Arc<Mutex<TestContext>>;

/// Extracts values from the locked context, executes an async operation,
/// and optionally updates the context with results.
fn with_context_async<F, R, U>(
    world: &SharedContext,
    extract: impl FnOnce(&TestContext) -> F,
    operation: impl FnOnce(F) -> R,
    update: U,
) where
    R: std::future::Future,
    U: FnOnce(&mut TestContext, R::Output),
{
    assert!(
        tokio::runtime::Handle::try_current().is_err(),
        "do not call with_context_async from inside a Tokio runtime"
    );

    let (handle, extracted) = {
        let ctx = world.lock().expect("context lock");
        (ctx.runtime.handle().clone(), extract(&ctx))
    };
    let result = handle.block_on(operation(extracted));
    let mut ctx = world.lock().expect("context lock");
    update(&mut ctx, result);
}

fn setup_test_context() -> Result<TestContext, String> {
    let runtime = Runtime::new().map_err(|err| err.to_string())?;
    let cluster = shared_cluster_handle().map_err(|e| e.to_string())?;
    let temp_db = provision_template_database(cluster).map_err(|err| err.to_string())?;

    let database_url = temp_db.url().to_string();

    // Create the SQLx pool for Apalis.
    let pool = runtime
        .block_on(async { PgPool::connect(&database_url).await })
        .map_err(|err| err.to_string())?;

    // Create the Apalis provider (which calls setup internally) and queue adapter.
    let provider = runtime
        .block_on(async { ApalisPostgresProvider::new(pool.clone()).await })
        .map_err(|err| err.to_string())?;
    let queue = GenericApalisRouteQueue::new(provider);

    Ok(TestContext {
        runtime,
        queue: Some(Arc::new(queue)),
        pool: Some(pool),
        enqueue_results: Vec::new(),
        enqueued_plans: Vec::new(),
        _database: temp_db,
    })
}

#[fixture]
fn world() -> Option<SharedContext> {
    match setup_test_context() {
        Ok(ctx) => Some(Arc::new(Mutex::new(ctx))),
        Err(reason) => handle_cluster_setup_failure(reason),
    }
}

// -- Step definitions --

#[given("a test database with Apalis storage initialised")]
fn a_test_database_with_apalis_storage_initialised(world: &Option<SharedContext>) {
    let Some(world) = world else { return };
    // Context setup already initialised the storage in the fixture.
    let ctx = world.lock().expect("context lock");
    assert!(ctx.queue.is_some(), "queue adapter should be initialised");
    assert!(ctx.pool.is_some(), "pool should be initialised");
}

#[given("the queue adapter uses an invalid database connection")]
fn the_queue_adapter_uses_an_invalid_database_connection(world: &Option<SharedContext>) {
    let Some(world) = world else { return };
    let mut ctx = world.lock().expect("context lock");
    // Replace the queue with a failing provider that simulates database unavailability.
    // This exercises the adapter's error mapping without requiring actual database failures.
    let failing_provider = FailingProvider::new("Database connection unavailable".to_string());
    ctx.queue = Some(Arc::new(GenericApalisRouteQueue::new(failing_provider)));
}

fn enqueue_test_plan_with_name(world: &SharedContext, name: String) {
    let plan = TestPlan { name };

    with_context_async(
        world,
        |ctx| {
            (
                ctx.queue.clone().expect("queue should be initialised"),
                plan.clone(),
            )
        },
        |(queue, plan_to_enqueue)| async move {
            let result = queue.enqueue(&plan_to_enqueue).await;
            (result, plan_to_enqueue)
        },
        |ctx, (result, plan_to_store)| {
            ctx.enqueued_plans.push(plan_to_store);
            ctx.enqueue_results.push(result);
        },
    );
}

#[when("I enqueue a test plan")]
fn i_enqueue_a_test_plan(world: &Option<SharedContext>) {
    let Some(world) = world else { return };
    enqueue_test_plan_with_name(world, "test-plan".to_string());
}

#[when("I enqueue the first test plan")]
fn i_enqueue_the_first_test_plan(world: &Option<SharedContext>) {
    let Some(world) = world else { return };
    enqueue_test_plan_with_name(world, "first-plan".to_string());
}

#[when("I enqueue the second test plan")]
fn i_enqueue_the_second_test_plan(world: &Option<SharedContext>) {
    let Some(world) = world else { return };
    enqueue_test_plan_with_name(world, "second-plan".to_string());
}

#[when("I enqueue the same test plan again")]
fn i_enqueue_the_same_test_plan_again(world: &Option<SharedContext>) {
    let Some(world) = world else { return };
    enqueue_test_plan_with_name(world, "test-plan".to_string());
}

#[when("I attempt to enqueue a test plan")]
fn i_attempt_to_enqueue_a_test_plan(world: &Option<SharedContext>) {
    let Some(world) = world else { return };
    enqueue_test_plan_with_name(world, "test-plan".to_string());
}

#[then("the enqueue operation succeeds")]
fn the_enqueue_operation_succeeds(world: &Option<SharedContext>) {
    let Some(world) = world else { return };
    let ctx = world.lock().expect("context lock");
    let last_result = ctx
        .enqueue_results
        .last()
        .expect("at least one enqueue result");
    assert!(
        last_result.is_ok(),
        "expected enqueue to succeed, got: {last_result:?}"
    );
}

#[then("both enqueue operations succeed")]
fn both_enqueue_operations_succeed(world: &Option<SharedContext>) {
    let Some(world) = world else { return };
    let ctx = world.lock().expect("context lock");
    assert_eq!(
        ctx.enqueue_results.len(),
        2,
        "expected exactly two enqueue results"
    );
    for (idx, result) in ctx.enqueue_results.iter().enumerate() {
        assert!(
            result.is_ok(),
            "expected enqueue {idx} to succeed, got: {result:?}"
        );
    }
}

#[then("the enqueue operation fails with an unavailable error")]
fn the_enqueue_operation_fails_with_an_unavailable_error(world: &Option<SharedContext>) {
    let Some(world) = world else { return };
    let ctx = world.lock().expect("context lock");
    let last_result = ctx
        .enqueue_results
        .last()
        .expect("at least one enqueue result");
    match last_result {
        Err(JobDispatchError::Unavailable { .. }) => {}
        other => panic!("expected Unavailable error, got: {other:?}"),
    }
}

/// Returns the number of Apalis jobs whose `name` field matches `plan_name`.
async fn count_jobs_for_plan_name(pool: &PgPool, plan_name: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM apalis.jobs WHERE convert_from(job, 'UTF8')::jsonb->>'name' = $1",
    )
    .bind(plan_name)
    .fetch_one(pool)
    .await
    .expect("query job count")
}

/// Fetches the job count for the most recently enqueued plan.
fn fetch_last_plan_job_count(world: &SharedContext) -> i64 {
    let mut job_count: i64 = 0;
    with_context_async(
        world,
        |ctx| {
            let pool = ctx.pool.clone().expect("pool should be available");
            let plan = ctx.enqueued_plans.last().expect("at least one plan");
            (pool, plan.clone())
        },
        |(pool, plan)| async move { count_jobs_for_plan_name(&pool, &plan.name).await },
        |_ctx, count| {
            job_count = count;
        },
    );
    job_count
}

#[then("the plan is persisted in the queue storage")]
fn the_plan_is_persisted_in_the_queue_storage(world: &Option<SharedContext>) {
    let Some(world) = world else { return };
    let count = fetch_last_plan_job_count(world);
    assert!(
        count >= 1,
        "expected at least one job in storage, found {count}"
    );
}

#[then("both plans are persisted as separate jobs")]
fn both_plans_are_persisted_as_separate_jobs(world: &Option<SharedContext>) {
    let Some(world) = world else { return };
    with_context_async(
        world,
        |ctx| {
            let pool = ctx.pool.clone().expect("pool should be available");
            let plans = ctx.enqueued_plans.clone();
            (pool, plans)
        },
        |(pool, plans)| async move {
            let mut counts = Vec::new();
            for plan in &plans {
                counts.push(count_jobs_for_plan_name(&pool, &plan.name).await);
            }
            counts
        },
        |_ctx, counts| {
            for (idx, count) in counts.iter().enumerate() {
                assert!(
                    *count >= 1,
                    "expected plan {idx} to have at least one job, found {count}"
                );
            }
        },
    );
}

#[then("two independent jobs exist in storage")]
fn two_independent_jobs_exist_in_storage(world: &Option<SharedContext>) {
    let Some(world) = world else { return };
    let count = fetch_last_plan_job_count(world);
    assert_eq!(
        count, 2,
        "expected exactly two jobs for duplicate plan, found {count}"
    );
}

// -- Scenario bindings --

#[scenario(path = "tests/features/route_queue_apalis.feature")]
#[rstest]
fn route_queue_apalis(world: Option<SharedContext>) {
    if world.is_none() {
        eprintln!("Skipping route_queue_apalis: cluster setup failed");
    }
}
