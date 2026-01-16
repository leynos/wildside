//! Integration tests for `DieselExampleDataRunsRepository` against embedded PostgreSQL.
//!
//! These tests verify that the Diesel-backed example data runs repository correctly
//! implements the `ExampleDataRunsRepository` port contract against a real PostgreSQL
//! database. Tests use `pg-embedded-setup-unpriv` for isolated database instances.
//!
//! # Runtime Strategy
//!
//! The rstest-bdd-macros crate does not support async step definitions, so we
//! store a Tokio runtime in the test context and reuse it for all async
//! operations. This avoids the overhead of creating a new runtime per async
//! block while maintaining BDD step compatibility.

use std::sync::{Arc, Mutex};

use backend::domain::ports::{ExampleDataRunsError, ExampleDataRunsRepository, SeedingResult};
use backend::outbound::persistence::{DbPool, DieselExampleDataRunsRepository, PoolConfig};
use pg_embedded_setup_unpriv::TemporaryDatabase;
use rstest::{fixture, rstest};
use rstest_bdd_macros::{given, then, when};
use tokio::runtime::Runtime;

#[path = "support/pg_embed.rs"]
mod pg_embed;

mod support;

use pg_embed::shared_cluster;
use support::{handle_cluster_setup_failure, provision_template_database};

// -----------------------------------------------------------------------------
// Fixtures
// -----------------------------------------------------------------------------

const TEST_SEED_KEY: &str = "test-seed";
const TEST_USER_COUNT: i32 = 12;
const TEST_SEED_VALUE: i64 = 2026;

const SECOND_SEED_KEY: &str = "second-seed";
const SECOND_USER_COUNT: i32 = 5;
const SECOND_SEED_VALUE: i64 = 42;

// -----------------------------------------------------------------------------
// Test Context
// -----------------------------------------------------------------------------

struct TestContext {
    /// Tokio runtime reused for all async operations in this test.
    runtime: Runtime,
    repository: DieselExampleDataRunsRepository,
    last_record_result: Option<Result<SeedingResult, ExampleDataRunsError>>,
    last_is_seeded_result: Option<Result<bool, ExampleDataRunsError>>,
    _database: TemporaryDatabase,
}

type SharedContext = Arc<Mutex<TestContext>>;

/// Extracts values from the locked context, executes an async operation,
/// and optionally updates the context with results.
fn with_context_async<F, R, U>(
    world: &SharedContext,
    extract: impl FnOnce(&TestContext) -> F,
    operation: impl FnOnce(DieselExampleDataRunsRepository, F) -> R,
    update: U,
) where
    R: std::future::Future,
    U: FnOnce(&mut TestContext, R::Output),
{
    assert!(
        tokio::runtime::Handle::try_current().is_err(),
        "do not call with_context_async from inside a Tokio runtime"
    );

    let (repo, handle, extracted) = {
        let ctx = world.lock().expect("context lock");
        (
            ctx.repository.clone(),
            ctx.runtime.handle().clone(),
            extract(&ctx),
        )
    };
    let result = handle.block_on(operation(repo, extracted));
    let mut ctx = world.lock().expect("context lock");
    update(&mut ctx, result);
}

fn setup_test_context() -> Result<TestContext, String> {
    let runtime = Runtime::new().map_err(|err| err.to_string())?;
    let cluster = shared_cluster()?;
    let temp_db = provision_template_database(cluster).map_err(|err| err.to_string())?;

    let database_url = temp_db.url().to_string();

    // Create the connection pool and repository.
    let config = PoolConfig::new(&database_url)
        .with_max_size(2)
        .with_min_idle(Some(1));

    let pool = runtime
        .block_on(async { DbPool::new(config).await })
        .map_err(|err| err.to_string())?;

    let repository = DieselExampleDataRunsRepository::new(pool);

    Ok(TestContext {
        runtime,
        repository,
        last_record_result: None,
        last_is_seeded_result: None,
        _database: temp_db,
    })
}

#[fixture]
fn diesel_world() -> Option<SharedContext> {
    match setup_test_context() {
        Ok(ctx) => Some(Arc::new(Mutex::new(ctx))),
        Err(reason) => handle_cluster_setup_failure(reason),
    }
}

// -----------------------------------------------------------------------------
// BDD Step Definitions
// -----------------------------------------------------------------------------

#[given("a Diesel-backed example data runs repository")]
fn a_diesel_backed_example_data_runs_repository(world: SharedContext) {
    // Context already initialised with repository.
    let _ = world;
}

#[when("the repository records a seed")]
fn the_repository_records_a_seed(world: SharedContext) {
    with_context_async(
        &world,
        |_| (TEST_SEED_KEY, TEST_USER_COUNT, TEST_SEED_VALUE),
        |repo, (seed_key, user_count, seed)| async move {
            repo.try_record_seed(seed_key, user_count, seed).await
        },
        |ctx, result| {
            ctx.last_record_result = Some(result);
        },
    );
}

#[when("the repository records the same seed again")]
fn the_repository_records_the_same_seed_again(world: SharedContext) {
    with_context_async(
        &world,
        |_| (TEST_SEED_KEY, TEST_USER_COUNT, TEST_SEED_VALUE),
        |repo, (seed_key, user_count, seed)| async move {
            repo.try_record_seed(seed_key, user_count, seed).await
        },
        |ctx, result| {
            ctx.last_record_result = Some(result);
        },
    );
}

#[when("the repository records a different seed")]
fn the_repository_records_a_different_seed(world: SharedContext) {
    with_context_async(
        &world,
        |_| (SECOND_SEED_KEY, SECOND_USER_COUNT, SECOND_SEED_VALUE),
        |repo, (seed_key, user_count, seed)| async move {
            repo.try_record_seed(seed_key, user_count, seed).await
        },
        |ctx, result| {
            ctx.last_record_result = Some(result);
        },
    );
}

#[when("the repository checks if seed exists")]
fn the_repository_checks_if_seed_exists(world: SharedContext) {
    with_context_async(
        &world,
        |_| TEST_SEED_KEY,
        |repo, seed_key| async move { repo.is_seeded(seed_key).await },
        |ctx, result| {
            ctx.last_is_seeded_result = Some(result);
        },
    );
}

#[when("the repository checks if unknown seed exists")]
fn the_repository_checks_if_unknown_seed_exists(world: SharedContext) {
    with_context_async(
        &world,
        |_| "nonexistent-seed",
        |repo, seed_key| async move { repo.is_seeded(seed_key).await },
        |ctx, result| {
            ctx.last_is_seeded_result = Some(result);
        },
    );
}

#[then("the result is applied")]
fn the_result_is_applied(world: SharedContext) {
    let ctx = world.lock().expect("context lock");
    let result = ctx
        .last_record_result
        .as_ref()
        .expect("record was executed");
    match result {
        Ok(SeedingResult::Applied) => {}
        Ok(SeedingResult::AlreadySeeded) => {
            panic!("expected Applied, got AlreadySeeded")
        }
        Err(err) => panic!("expected Applied, got error: {err}"),
    }
}

#[then("the result is already seeded")]
fn the_result_is_already_seeded(world: SharedContext) {
    let ctx = world.lock().expect("context lock");
    let result = ctx
        .last_record_result
        .as_ref()
        .expect("record was executed");
    match result {
        Ok(SeedingResult::AlreadySeeded) => {}
        Ok(SeedingResult::Applied) => {
            panic!("expected AlreadySeeded, got Applied")
        }
        Err(err) => panic!("expected AlreadySeeded, got error: {err}"),
    }
}

#[then("is seeded returns true")]
fn is_seeded_returns_true(world: SharedContext) {
    let ctx = world.lock().expect("context lock");
    let result = ctx
        .last_is_seeded_result
        .as_ref()
        .expect("is_seeded was executed");
    match result {
        Ok(true) => {}
        Ok(false) => panic!("expected is_seeded=true, got false"),
        Err(err) => panic!("expected is_seeded=true, got error: {err}"),
    }
}

#[then("is seeded returns false")]
fn is_seeded_returns_false(world: SharedContext) {
    let ctx = world.lock().expect("context lock");
    let result = ctx
        .last_is_seeded_result
        .as_ref()
        .expect("is_seeded was executed");
    match result {
        Ok(false) => {}
        Ok(true) => panic!("expected is_seeded=false, got true"),
        Err(err) => panic!("expected is_seeded=false, got error: {err}"),
    }
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[rstest]
fn try_record_seed_returns_applied_on_first_insert(diesel_world: Option<SharedContext>) {
    let Some(world) = diesel_world else {
        eprintln!("SKIP-TEST-CLUSTER: try_record_seed_returns_applied_on_first_insert skipped");
        return;
    };

    a_diesel_backed_example_data_runs_repository(world.clone());
    the_repository_records_a_seed(world.clone());
    the_result_is_applied(world);
}

#[rstest]
fn try_record_seed_returns_already_seeded_on_duplicate(diesel_world: Option<SharedContext>) {
    let Some(world) = diesel_world else {
        eprintln!("SKIP-TEST-CLUSTER: try_record_seed_returns_already_seeded_on_duplicate skipped");
        return;
    };

    a_diesel_backed_example_data_runs_repository(world.clone());
    the_repository_records_a_seed(world.clone());
    the_result_is_applied(world.clone());

    the_repository_records_the_same_seed_again(world.clone());
    the_result_is_already_seeded(world);
}

#[rstest]
fn different_seeds_are_independent(diesel_world: Option<SharedContext>) {
    let Some(world) = diesel_world else {
        eprintln!("SKIP-TEST-CLUSTER: different_seeds_are_independent skipped");
        return;
    };

    a_diesel_backed_example_data_runs_repository(world.clone());

    // First seed
    the_repository_records_a_seed(world.clone());
    the_result_is_applied(world.clone());

    // Second seed - should also be applied (different key)
    the_repository_records_a_different_seed(world.clone());
    the_result_is_applied(world);
}

#[rstest]
fn is_seeded_returns_false_for_unknown_seed(diesel_world: Option<SharedContext>) {
    let Some(world) = diesel_world else {
        eprintln!("SKIP-TEST-CLUSTER: is_seeded_returns_false_for_unknown_seed skipped");
        return;
    };

    a_diesel_backed_example_data_runs_repository(world.clone());
    the_repository_checks_if_unknown_seed_exists(world.clone());
    is_seeded_returns_false(world);
}

#[rstest]
fn is_seeded_returns_true_after_recording(diesel_world: Option<SharedContext>) {
    let Some(world) = diesel_world else {
        eprintln!("SKIP-TEST-CLUSTER: is_seeded_returns_true_after_recording skipped");
        return;
    };

    a_diesel_backed_example_data_runs_repository(world.clone());

    // Initially not seeded
    the_repository_checks_if_seed_exists(world.clone());
    is_seeded_returns_false(world.clone());

    // Record the seed
    the_repository_records_a_seed(world.clone());
    the_result_is_applied(world.clone());

    // Now seeded
    the_repository_checks_if_seed_exists(world.clone());
    is_seeded_returns_true(world);
}
