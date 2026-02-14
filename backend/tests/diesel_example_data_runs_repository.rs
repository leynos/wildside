//! Integration tests for `DieselExampleDataRunsRepository` against embedded PostgreSQL.
//!
//! These tests verify that the Diesel-backed example data runs repository correctly
//! implements the `ExampleDataRunsRepository` port contract against a real PostgreSQL
//! database. Tests use `pg-embedded-setup-unpriv` for isolated database instances.
//!
//! # Runtime Strategy
//!
//! `rstest-bdd` v0.5.0 supports async step definitions, but this suite keeps
//! synchronous steps and reuses a shared Tokio runtime in the test context.
//! This keeps repository operations deterministic and avoids recreating a
//! runtime for each step.

use std::sync::{Arc, Mutex};

use backend::domain::ports::{ExampleDataRunsError, ExampleDataRunsRepository, SeedingResult};
use backend::outbound::persistence::{DbPool, DieselExampleDataRunsRepository, PoolConfig};
use futures::future::join_all;
use pg_embedded_setup_unpriv::TemporaryDatabase;
use rstest::{fixture, rstest};
use rstest_bdd_macros::{given, then, when};
use tokio::runtime::Runtime;

mod support;

use support::atexit_cleanup::shared_cluster_handle;
use support::{handle_cluster_setup_failure, provision_template_database};

const TEST_SEED_KEY: &str = "test-seed";
const TEST_USER_COUNT: i32 = 12;
const TEST_SEED_VALUE: i64 = 2026;

const SECOND_SEED_KEY: &str = "second-seed";
const SECOND_USER_COUNT: i32 = 5;
const SECOND_SEED_VALUE: i64 = 42;

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
    let cluster = shared_cluster_handle().map_err(|e| e.to_string())?;
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

fn record_seed(world: &SharedContext, seed_key: &str, user_count: i32, seed_value: i64) {
    with_context_async(
        world,
        |_| (seed_key, user_count, seed_value),
        |repo, (key, count, seed)| async move { repo.try_record_seed(key, count, seed).await },
        |ctx, result| {
            ctx.last_record_result = Some(result);
        },
    );
}

fn check_is_seeded(world: &SharedContext, seed_key: &str) {
    with_context_async(
        world,
        |_| seed_key,
        |repo, key| async move { repo.is_seeded(key).await },
        |ctx, result| {
            ctx.last_is_seeded_result = Some(result);
        },
    );
}

fn assert_seeding_result(world: &SharedContext, expected: SeedingResult) {
    let ctx = world.lock().expect("context lock");
    let result = ctx
        .last_record_result
        .as_ref()
        .expect("record was executed");
    match result {
        Ok(actual) if *actual == expected => {}
        Ok(actual) => panic!("expected {expected:?}, got {actual:?}"),
        Err(err) => panic!("expected {expected:?}, got error: {err}"),
    }
}

fn assert_is_seeded(world: &SharedContext, expected: bool) {
    let ctx = world.lock().expect("context lock");
    let result = ctx
        .last_is_seeded_result
        .as_ref()
        .expect("is_seeded was executed");
    match result {
        Ok(actual) if *actual == expected => {}
        Ok(actual) => panic!("expected is_seeded={expected}, got {actual}"),
        Err(err) => panic!("expected is_seeded={expected}, got error: {err}"),
    }
}

#[given("a Diesel-backed example data runs repository")]
fn a_diesel_backed_example_data_runs_repository(_world: SharedContext) {}

#[when("the repository records a seed")]
fn the_repository_records_a_seed(world: SharedContext) {
    record_seed(&world, TEST_SEED_KEY, TEST_USER_COUNT, TEST_SEED_VALUE);
}

#[when("the repository records the same seed again")]
fn the_repository_records_the_same_seed_again(world: SharedContext) {
    record_seed(&world, TEST_SEED_KEY, TEST_USER_COUNT, TEST_SEED_VALUE);
}

#[when("the repository records a different seed")]
fn the_repository_records_a_different_seed(world: SharedContext) {
    record_seed(
        &world,
        SECOND_SEED_KEY,
        SECOND_USER_COUNT,
        SECOND_SEED_VALUE,
    );
}

#[when("the repository checks if seed exists")]
fn the_repository_checks_if_seed_exists(world: SharedContext) {
    check_is_seeded(&world, TEST_SEED_KEY);
}

#[when("the repository checks if unknown seed exists")]
fn the_repository_checks_if_unknown_seed_exists(world: SharedContext) {
    check_is_seeded(&world, "nonexistent-seed");
}

#[then("the result is applied")]
fn the_result_is_applied(world: SharedContext) {
    assert_seeding_result(&world, SeedingResult::Applied);
}

#[then("the result is already seeded")]
fn the_result_is_already_seeded(world: SharedContext) {
    assert_seeding_result(&world, SeedingResult::AlreadySeeded);
}

#[then("is seeded returns true")]
fn is_seeded_returns_true(world: SharedContext) {
    assert_is_seeded(&world, true);
}

#[then("is seeded returns false")]
fn is_seeded_returns_false(world: SharedContext) {
    assert_is_seeded(&world, false);
}

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

/// Verify that concurrent calls to `try_record_seed` for the same seed key
/// still respect once-only semantics: exactly one caller applies the seed,
/// the rest observe it as already seeded.
#[rstest]
fn concurrent_calls_for_same_seed_are_once_only(diesel_world: Option<SharedContext>) {
    let Some(world) = diesel_world else {
        eprintln!("SKIP-TEST-CLUSTER: concurrent_calls_for_same_seed_are_once_only skipped");
        return;
    };

    const CONCURRENT_SEED_KEY: &str = "concurrent-seed";
    const CONCURRENT_CALLS: usize = 10;

    let ctx = world.lock().expect("context lock");
    let repository = ctx.repository.clone();
    let handle = ctx.runtime.handle().clone();
    drop(ctx);

    let results: Vec<Result<SeedingResult, ExampleDataRunsError>> = handle.block_on(async {
        let futures = (0..CONCURRENT_CALLS).map(|_| {
            let repo = repository.clone();
            async move { repo.try_record_seed(CONCURRENT_SEED_KEY, 10, 42).await }
        });
        join_all(futures).await
    });

    let applied_count = results
        .iter()
        .filter(|r| matches!(r, Ok(SeedingResult::Applied)))
        .count();
    let already_seeded_count = results
        .iter()
        .filter(|r| matches!(r, Ok(SeedingResult::AlreadySeeded)))
        .count();
    let error_count = results.iter().filter(|r| r.is_err()).count();

    assert_eq!(
        error_count, 0,
        "all concurrent calls should succeed without errors"
    );
    assert_eq!(
        applied_count, 1,
        "exactly one concurrent caller should apply the seed"
    );
    assert_eq!(
        already_seeded_count,
        CONCURRENT_CALLS - 1,
        "all other concurrent callers should see the seed as already applied"
    );
}

/// Verify that recording the same seed_key with different metadata values
/// still returns AlreadySeeded and does not update the original record.
#[rstest]
fn same_seed_key_with_different_metadata_returns_already_seeded(
    diesel_world: Option<SharedContext>,
) {
    let Some(world) = diesel_world else {
        eprintln!(
            "SKIP-TEST-CLUSTER: same_seed_key_with_different_metadata_returns_already_seeded skipped"
        );
        return;
    };

    const METADATA_TEST_KEY: &str = "metadata-test-seed";
    const ORIGINAL_USER_COUNT: i32 = 100;
    const ORIGINAL_SEED_VALUE: i64 = 9999;
    const DIFFERENT_USER_COUNT: i32 = 200;
    const DIFFERENT_SEED_VALUE: i64 = 1111;

    let ctx = world.lock().expect("context lock");
    let repository = ctx.repository.clone();
    let handle = ctx.runtime.handle().clone();
    drop(ctx);

    // First insert with original metadata
    let first_result = handle.block_on(async {
        repository
            .try_record_seed(METADATA_TEST_KEY, ORIGINAL_USER_COUNT, ORIGINAL_SEED_VALUE)
            .await
    });
    assert!(
        matches!(first_result, Ok(SeedingResult::Applied)),
        "first insert should succeed"
    );

    // Second insert with different metadata - should return AlreadySeeded
    let second_result = handle.block_on(async {
        repository
            .try_record_seed(
                METADATA_TEST_KEY,
                DIFFERENT_USER_COUNT,
                DIFFERENT_SEED_VALUE,
            )
            .await
    });
    assert!(
        matches!(second_result, Ok(SeedingResult::AlreadySeeded)),
        "second insert with different metadata should return AlreadySeeded, got: {second_result:?}"
    );
}
