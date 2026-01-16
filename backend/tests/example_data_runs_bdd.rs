//! Behaviour tests for example data seeding guard.
//!
//! These scenarios validate that the repository correctly guards against
//! duplicate seeding, allowing exactly-once semantics even under concurrent
//! startup attempts.

use std::cell::RefCell;

use backend::domain::ports::{ExampleDataRunsError, ExampleDataRunsRepository, SeedingResult};
use backend::outbound::persistence::{DbPool, DieselExampleDataRunsRepository, PoolConfig};
use pg_embedded_setup_unpriv::TemporaryDatabase;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tokio::runtime::Runtime;

#[path = "support/pg_embed.rs"]
mod pg_embed;

mod support;

use pg_embed::shared_cluster;
use support::{handle_cluster_setup_failure, provision_template_database};

// -----------------------------------------------------------------------------
// Test World
// -----------------------------------------------------------------------------

struct ExampleDataRunsWorld {
    runtime: RefCell<Option<Runtime>>,
    repository: RefCell<Option<DieselExampleDataRunsRepository>>,
    last_record_result: RefCell<Option<Result<SeedingResult, ExampleDataRunsError>>>,
    last_is_seeded_result: RefCell<Option<Result<bool, ExampleDataRunsError>>>,
    _database: RefCell<Option<TemporaryDatabase>>,
    setup_error: RefCell<Option<String>>,
}

impl ExampleDataRunsWorld {
    fn new() -> Self {
        Self {
            runtime: RefCell::new(None),
            repository: RefCell::new(None),
            last_record_result: RefCell::new(None),
            last_is_seeded_result: RefCell::new(None),
            _database: RefCell::new(None),
            setup_error: RefCell::new(None),
        }
    }

    fn setup_fresh_database(&self) {
        let runtime = Runtime::new().expect("create runtime");

        let cluster = match shared_cluster() {
            Ok(c) => c,
            Err(reason) => {
                let _: Option<()> = handle_cluster_setup_failure(reason.clone());
                *self.setup_error.borrow_mut() = Some(reason);
                return;
            }
        };

        let temp_db = match provision_template_database(cluster) {
            Ok(db) => db,
            Err(err) => {
                let _: Option<()> = handle_cluster_setup_failure(err.to_string());
                *self.setup_error.borrow_mut() = Some(err.to_string());
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

        *self.runtime.borrow_mut() = Some(runtime);
        *self.repository.borrow_mut() = Some(repository);
        *self._database.borrow_mut() = Some(temp_db);
    }

    fn is_skipped(&self) -> bool {
        self.setup_error.borrow().is_some()
    }

    fn record_seed(&self, seed_key: &str) {
        if self.is_skipped() {
            return;
        }

        let runtime = self.runtime.borrow();
        let runtime = runtime.as_ref().expect("runtime");
        let repo = self.repository.borrow();
        let repo = repo.as_ref().expect("repository");

        let result = runtime.block_on(async { repo.try_record_seed(seed_key, 12, 2026).await });
        *self.last_record_result.borrow_mut() = Some(result);
    }

    fn check_seed_exists(&self, seed_key: &str) {
        if self.is_skipped() {
            return;
        }

        let runtime = self.runtime.borrow();
        let runtime = runtime.as_ref().expect("runtime");
        let repo = self.repository.borrow();
        let repo = repo.as_ref().expect("repository");

        let result = runtime.block_on(async { repo.is_seeded(seed_key).await });
        *self.last_is_seeded_result.borrow_mut() = Some(result);
    }

    fn assert_is_seeded_result(&self, expected: bool) {
        if self.is_skipped() {
            eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
            return;
        }

        let result = self.last_is_seeded_result.borrow();
        let result = result.as_ref().expect("is_seeded result should be set");

        match (result, expected) {
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
    ExampleDataRunsWorld::new()
}

// -----------------------------------------------------------------------------
// Given Steps
// -----------------------------------------------------------------------------

#[given("a fresh database")]
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

    let result = world.last_record_result.borrow();
    let result = result.as_ref().expect("record result should be set");

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
// Scenario Binding
// -----------------------------------------------------------------------------

#[scenario(path = "tests/features/example_data_runs.feature")]
fn example_data_runs_scenarios(world: ExampleDataRunsWorld) {
    drop(world);
}
