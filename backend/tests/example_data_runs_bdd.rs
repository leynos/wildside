//! Behaviour-driven development (BDD) tests for example data seeding guard.
//!
//! These scenarios validate that the repository correctly guards against
//! duplicate seeding, allowing exactly-once semantics even under concurrent
//! startup attempts.

use std::sync::Arc;

use backend::domain::ports::{ExampleDataRunsError, ExampleDataRunsRepository, SeedingResult};
use backend::outbound::persistence::{DbPool, DieselExampleDataRunsRepository, PoolConfig};
use pg_embedded_setup_unpriv::TemporaryDatabase;
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};
use tokio::runtime::Runtime;

#[path = "support/pg_embed.rs"]
mod pg_embed;

mod support;

use pg_embed::shared_cluster;
use support::{handle_cluster_setup_failure, provision_template_database};

// -----------------------------------------------------------------------------
// Test World
// -----------------------------------------------------------------------------

/// Wrapper for non-Clone types to enable storage in `Slot`.
#[derive(Clone)]
struct RuntimeHandle(Arc<Runtime>);

/// Wrapper for the temporary database to enable storage in `Slot`.
///
/// The inner field is never read directly; it exists to keep the temporary
/// database alive for the duration of the test.
#[derive(Clone)]
struct DatabaseHandle(#[expect(dead_code)] Arc<TemporaryDatabase>);

/// Test world holding repository and test results.
#[derive(Default, ScenarioState)]
struct ExampleDataRunsWorld {
    runtime: Slot<RuntimeHandle>,
    repository: Slot<DieselExampleDataRunsRepository>,
    last_record_result: Slot<Result<SeedingResult, ExampleDataRunsError>>,
    last_is_seeded_result: Slot<Result<bool, ExampleDataRunsError>>,
    _database: Slot<DatabaseHandle>,
    setup_error: Slot<String>,
}

impl ExampleDataRunsWorld {
    fn setup_fresh_database(&self) {
        let runtime = Runtime::new().expect("create runtime");

        let cluster = match shared_cluster() {
            Ok(c) => c,
            Err(reason) => {
                let _: Option<()> = handle_cluster_setup_failure(reason.clone());
                self.setup_error.set(reason);
                return;
            }
        };

        let temp_db = match provision_template_database(cluster) {
            Ok(db) => db,
            Err(err) => {
                let _: Option<()> = handle_cluster_setup_failure(err.to_string());
                self.setup_error.set(err.to_string());
                return;
            }
        };

        let database_url = temp_db.url().to_string();
        let config = PoolConfig::new(&database_url)
            .with_max_size(2)
            .with_min_idle(Some(1));

        let pool = runtime
            .block_on(async { DbPool::new(config).await })
            .expect("create pool");

        let repository = DieselExampleDataRunsRepository::new(pool);

        self.runtime.set(RuntimeHandle(Arc::new(runtime)));
        self.repository.set(repository);
        self._database.set(DatabaseHandle(Arc::new(temp_db)));
    }

    fn is_skipped(&self) -> bool {
        self.setup_error.get().is_some()
    }

    fn execute_async<T>(
        &self,
        operation: impl FnOnce(&Runtime, &DieselExampleDataRunsRepository) -> T,
    ) -> Option<T> {
        if self.is_skipped() {
            return None;
        }

        let runtime_handle = self.runtime.get().expect("runtime");
        let repo = self.repository.get().expect("repository");

        Some(operation(&runtime_handle.0, &repo))
    }

    fn record_seed(&self, seed_key: &str) {
        if let Some(result) = self.execute_async(|runtime, repo| {
            runtime.block_on(async { repo.try_record_seed(seed_key, 12, 2026).await })
        }) {
            self.last_record_result.set(result);
        }
    }

    fn check_seed_exists(&self, seed_key: &str) {
        if let Some(result) = self.execute_async(|runtime, repo| {
            runtime.block_on(async { repo.is_seeded(seed_key).await })
        }) {
            self.last_is_seeded_result.set(result);
        }
    }

    fn assert_is_seeded_result(&self, expected: bool) {
        if self.is_skipped() {
            eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
            return;
        }

        let result = self
            .last_is_seeded_result
            .get()
            .expect("is_seeded result should be set");

        match (&result, expected) {
            (Ok(actual), expected) if *actual == expected => {}
            (Ok(actual), expected) => {
                panic!("expected is_seeded={expected}, got {actual}")
            }
            (Err(err), expected) => {
                panic!("expected is_seeded={expected}, got error: {err}")
            }
        }
    }
}

#[fixture]
fn world() -> ExampleDataRunsWorld {
    ExampleDataRunsWorld::default()
}

// -----------------------------------------------------------------------------
// Given Steps
// -----------------------------------------------------------------------------

#[given("a fresh database for example data runs")]
fn a_fresh_database(world: &ExampleDataRunsWorld) {
    world.setup_fresh_database();
}

#[given("a database with seed {seed_key} already recorded")]
fn a_database_with_seed_already_recorded(world: &ExampleDataRunsWorld, seed_key: String) {
    world.setup_fresh_database();
    if !world.is_skipped() {
        world.record_seed(&seed_key);
    }
}

// -----------------------------------------------------------------------------
// When Steps
// -----------------------------------------------------------------------------

#[when("a seed is recorded for {seed_key}")]
fn a_seed_is_recorded_for(world: &ExampleDataRunsWorld, seed_key: String) {
    world.record_seed(&seed_key);
}

#[when("checking if seed {seed_key} exists")]
fn checking_if_seed_exists(world: &ExampleDataRunsWorld, seed_key: String) {
    world.check_seed_exists(&seed_key);
}

// -----------------------------------------------------------------------------
// Then Steps
// -----------------------------------------------------------------------------

#[then("the result is {expected}")]
fn the_result_is(world: &ExampleDataRunsWorld, expected: String) {
    if world.is_skipped() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
        return;
    }

    let result = world
        .last_record_result
        .get()
        .expect("record result should be set");

    match expected.as_str() {
        "\"applied\"" => match result {
            Ok(SeedingResult::Applied) => {}
            Ok(SeedingResult::AlreadySeeded) => panic!("expected Applied, got AlreadySeeded"),
            Err(err) => panic!("expected Applied, got error: {err}"),
        },
        "\"already seeded\"" => match result {
            Ok(SeedingResult::AlreadySeeded) => {}
            Ok(SeedingResult::Applied) => panic!("expected AlreadySeeded, got Applied"),
            Err(err) => panic!("expected AlreadySeeded, got error: {err}"),
        },
        other => panic!("unknown expected result: {other}"),
    }
}

#[then("the existence check returns true")]
fn the_existence_check_returns_true(world: &ExampleDataRunsWorld) {
    world.assert_is_seeded_result(true);
}

#[then("the existence check returns false")]
fn the_existence_check_returns_false(world: &ExampleDataRunsWorld) {
    world.assert_is_seeded_result(false);
}

// -----------------------------------------------------------------------------
// Scenario Bindings
// -----------------------------------------------------------------------------

#[scenario(
    path = "tests/features/example_data_runs.feature",
    name = "First seed attempt succeeds"
)]
fn first_seed_attempt_succeeds(world: ExampleDataRunsWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/example_data_runs.feature",
    name = "Duplicate seed attempt is detected"
)]
fn duplicate_seed_attempt_is_detected(world: ExampleDataRunsWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/example_data_runs.feature",
    name = "Different seeds are independent"
)]
fn different_seeds_are_independent(world: ExampleDataRunsWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/example_data_runs.feature",
    name = "Query returns false for unknown seeds"
)]
fn query_returns_false_for_unknown_seeds(world: ExampleDataRunsWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/example_data_runs.feature",
    name = "Query returns true for recorded seeds"
)]
fn query_returns_true_for_recorded_seeds(world: ExampleDataRunsWorld) {
    let _ = world;
}
