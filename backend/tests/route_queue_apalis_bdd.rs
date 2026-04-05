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

use backend::domain::ports::{JobDispatchError, RouteQueue};
use backend::outbound::queue::{ApalisPostgresProvider, GenericApalisRouteQueue};
use pg_embedded_setup_unpriv::TemporaryDatabase;
use rstest::{fixture, rstest};
use rstest_bdd_macros::{given, scenario, then, when};
use serde::{Deserialize, Serialize};
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

struct TestContext {
    /// Tokio runtime reused for all async operations in this test.
    runtime: Runtime,
    /// The queue adapter under test (or None if using invalid connection).
    queue: Option<GenericApalisRouteQueue<TestPlan, ApalisPostgresProvider>>,
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
        queue: Some(queue),
        pool: Some(pool),
        enqueue_results: Vec::new(),
        enqueued_plans: Vec::new(),
        _database: temp_db,
    })
}

#[fixture]
fn queue_world() -> Option<SharedContext> {
    match setup_test_context() {
        Ok(ctx) => Some(Arc::new(Mutex::new(ctx))),
        Err(reason) => handle_cluster_setup_failure(reason),
    }
}

// -- Step definitions --

#[given("a test database with Apalis storage initialised")]
fn a_test_database_with_apalis_storage_initialised(world: &SharedContext) {
    // Context setup already initialised the storage in the fixture.
    let ctx = world.lock().expect("context lock");
    assert!(ctx.queue.is_some(), "queue adapter should be initialised");
    assert!(ctx.pool.is_some(), "pool should be initialised");
}

#[given("the queue adapter uses an invalid database connection")]
fn the_queue_adapter_uses_an_invalid_database_connection(world: &SharedContext) {
    let mut ctx = world.lock().expect("context lock");
    // Replace the queue with one using an invalid connection.
    let invalid_url = "postgres://invalid:invalid@invalid:5432/invalid";
    let pool_result = ctx
        .runtime
        .block_on(async { PgPool::connect(invalid_url).await });

    match pool_result {
        Ok(pool) => {
            // If connection somehow succeeded, create provider but setup will fail.
            let provider_result = ctx
                .runtime
                .block_on(async { ApalisPostgresProvider::new(pool).await });
            match provider_result {
                Ok(provider) => {
                    ctx.queue = Some(GenericApalisRouteQueue::new(provider));
                }
                Err(_) => {
                    // Provider creation failed as expected.
                    ctx.queue = None;
                }
            }
        }
        Err(_) => {
            // Connection failed as expected - queue operations will fail.
            ctx.queue = None;
        }
    }
}

fn enqueue_test_plan_with_name(world: &SharedContext, name: String) {
    let plan = TestPlan { name };

    with_context_async(
        world,
        |ctx| (ctx.queue.clone(), plan.clone()),
        |(queue_opt, plan_to_enqueue)| async move {
            let result = match queue_opt {
                Some(queue) => queue.enqueue(&plan_to_enqueue).await,
                None => Err(JobDispatchError::unavailable("no queue available")),
            };
            (result, plan_to_enqueue)
        },
        |ctx, (result, plan_to_store)| {
            ctx.enqueued_plans.push(plan_to_store);
            ctx.enqueue_results.push(result);
        },
    );
}

#[when("I enqueue a test plan")]
fn i_enqueue_a_test_plan(world: &SharedContext) {
    enqueue_test_plan_with_name(world, "test-plan".to_string());
}

#[when("I enqueue the first test plan")]
fn i_enqueue_the_first_test_plan(world: &SharedContext) {
    enqueue_test_plan_with_name(world, "first-plan".to_string());
}

#[when("I enqueue the second test plan")]
fn i_enqueue_the_second_test_plan(world: &SharedContext) {
    enqueue_test_plan_with_name(world, "second-plan".to_string());
}

#[when("I enqueue the same test plan again")]
fn i_enqueue_the_same_test_plan_again(world: &SharedContext) {
    enqueue_test_plan_with_name(world, "duplicate-plan".to_string());
}

#[when("I attempt to enqueue a test plan")]
fn i_attempt_to_enqueue_a_test_plan(world: &SharedContext) {
    i_enqueue_a_test_plan(world);
}

#[then("the enqueue operation succeeds")]
fn the_enqueue_operation_succeeds(world: &SharedContext) {
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
fn both_enqueue_operations_succeed(world: &SharedContext) {
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
fn the_enqueue_operation_fails_with_an_unavailable_error(world: &SharedContext) {
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

#[then("the plan is persisted in the queue storage")]
fn the_plan_is_persisted_in_the_queue_storage(world: &SharedContext) {
    with_context_async(
        world,
        |ctx| {
            let pool = ctx.pool.clone().expect("pool should be available");
            let plan = ctx.enqueued_plans.last().expect("at least one plan");
            (pool, plan.clone())
        },
        |(pool, plan)| async move {
            let count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM apalis.jobs WHERE job->>'name' = $1")
                    .bind(&plan.name)
                    .fetch_one(&pool)
                    .await
                    .expect("query job count");
            count
        },
        |_ctx, count| {
            assert!(
                count >= 1,
                "expected at least one job in storage, found {count}"
            );
        },
    );
}

#[then("both plans are persisted as separate jobs")]
fn both_plans_are_persisted_as_separate_jobs(world: &SharedContext) {
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
                let count: i64 =
                    sqlx::query_scalar("SELECT COUNT(*) FROM apalis.jobs WHERE job->>'name' = $1")
                        .bind(&plan.name)
                        .fetch_one(&pool)
                        .await
                        .expect("query job count");
                counts.push(count);
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
fn two_independent_jobs_exist_in_storage(world: &SharedContext) {
    with_context_async(
        world,
        |ctx| {
            let pool = ctx.pool.clone().expect("pool should be available");
            let plan = ctx.enqueued_plans.last().expect("at least one plan");
            (pool, plan.clone())
        },
        |(pool, plan)| async move {
            let count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM apalis.jobs WHERE job->>'name' = $1")
                    .bind(&plan.name)
                    .fetch_one(&pool)
                    .await
                    .expect("query job count");
            count
        },
        |_ctx, count| {
            assert_eq!(
                count, 2,
                "expected exactly two jobs for duplicate plan, found {count}"
            );
        },
    );
}

// -- Scenario bindings --

#[scenario(path = "tests/features/route_queue_apalis.feature")]
#[rstest]
fn route_queue_apalis(queue_world: Option<SharedContext>) {
    if queue_world.is_none() {
        eprintln!("Skipping route_queue_apalis: cluster setup failed");
    }
}
