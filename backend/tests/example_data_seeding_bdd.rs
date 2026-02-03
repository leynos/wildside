#![cfg(feature = "example-data")]
//! Behaviour-driven tests for example data startup seeding.
//!
//! These scenarios validate that the startup seeding flow applies example
//! users once, skips repeat runs, and reports errors for missing seeds.

use std::sync::Arc;

use backend::domain::{ExampleDataSeedOutcome, ExampleDataSeeder, ExampleDataSeedingError};
use backend::outbound::persistence::{DbPool, DieselExampleDataSeedRepository, PoolConfig};
use diesel::QueryableByName;
use diesel::sql_query;
use diesel::sql_types::BigInt;
use diesel_async::RunQueryDsl;
use example_data::SeedRegistry;
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

/// Wrapper for non-Clone runtime handle.
#[derive(Clone)]
struct RuntimeHandle(Arc<Runtime>);

/// Wrapper for the temporary database handle.
#[derive(Clone)]
struct DatabaseHandle(#[expect(dead_code)] Arc<TemporaryDatabase>);

/// Count row for raw SQL queries.
#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[derive(Default, ScenarioState)]
struct ExampleDataSeedingWorld {
    runtime: Slot<RuntimeHandle>,
    pool: Slot<DbPool>,
    seeder: Slot<ExampleDataSeeder<DieselExampleDataSeedRepository>>,
    registry: Slot<SeedRegistry>,
    last_result: Slot<Result<ExampleDataSeedOutcome, Arc<ExampleDataSeedingError>>>,
    last_user_count: Slot<i64>,
    last_preferences_count: Slot<i64>,
    _database: Slot<DatabaseHandle>,
    setup_error: Slot<String>,
}

impl ExampleDataSeedingWorld {
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

        let repository = DieselExampleDataSeedRepository::new(pool.clone());
        let seeder = ExampleDataSeeder::new(Arc::new(repository));

        self.runtime.set(RuntimeHandle(Arc::new(runtime)));
        self.pool.set(pool);
        self.seeder.set(seeder);
        self._database.set(DatabaseHandle(Arc::new(temp_db)));
    }

    fn is_skipped(&self) -> bool {
        self.setup_error.get().is_some()
    }

    fn set_registry(&self, seed_key: &str) {
        let seed_key = seed_key.trim_matches('"');
        let registry_json = format!(
            r#"{{
                "version": 1,
                "interestThemeIds": ["3fa85f64-5717-4562-b3fc-2c963f66afa6"],
                "safetyToggleIds": [],
                "seeds": [{{"name": "{seed_key}", "seed": 42, "userCount": 2}}]
            }}"#
        );
        let registry = SeedRegistry::from_json(&registry_json).expect("registry should parse");
        self.registry.set(registry);
    }

    fn execute_async<T>(
        &self,
        operation: impl FnOnce(
            &Runtime,
            &DbPool,
            &ExampleDataSeeder<DieselExampleDataSeedRepository>,
            &SeedRegistry,
        ) -> T,
    ) -> Option<T> {
        if self.is_skipped() {
            return None;
        }

        let runtime = self.runtime.get().expect("runtime");
        let pool = self.pool.get().expect("pool");
        let seeder = self.seeder.get().expect("seeder");
        let registry = self.registry.get().expect("registry");

        Some(operation(&runtime.0, &pool, &seeder, &registry))
    }

    fn run_seeding(&self, seed_key: &str) {
        let seed_key = seed_key.trim_matches('"');
        if let Some(result) = self.execute_async(|runtime, _, seeder, registry| {
            runtime
                .block_on(async { seeder.seed_from_registry(registry, seed_key, None).await })
                .map_err(Arc::new)
        }) {
            self.last_result.set(result);
        }
    }

    fn record_table_counts(&self) {
        if let Some((users, prefs)) = self.execute_async(|runtime, pool, _, _| {
            runtime.block_on(async {
                let users = count_table(pool, "users").await;
                let prefs = count_table(pool, "user_preferences").await;
                (users, prefs)
            })
        }) {
            self.last_user_count.set(users);
            self.last_preferences_count.set(prefs);
        }
    }
}

async fn count_table(pool: &DbPool, table: &str) -> i64 {
    let mut conn = pool.get().await.expect("get connection");
    let query = format!("SELECT COUNT(*) AS count FROM {table}");
    let row: CountRow = sql_query(query)
        .get_result(&mut conn)
        .await
        .expect("count query");
    row.count
}

#[fixture]
fn world() -> ExampleDataSeedingWorld {
    ExampleDataSeedingWorld::default()
}

// -----------------------------------------------------------------------------
// Given Steps
// -----------------------------------------------------------------------------

#[given("a fresh database")]
fn a_fresh_database(world: &ExampleDataSeedingWorld) {
    world.setup_fresh_database();
}

#[given("a seed registry with seed {seed_key}")]
fn a_seed_registry_with_seed(world: &ExampleDataSeedingWorld, seed_key: String) {
    world.set_registry(&seed_key);
}

// -----------------------------------------------------------------------------
// When Steps
// -----------------------------------------------------------------------------

#[when("startup seeding runs for {seed_key}")]
fn startup_seeding_runs_for(world: &ExampleDataSeedingWorld, seed_key: String) {
    world.run_seeding(&seed_key);
}

#[when("startup seeding runs again for {seed_key}")]
fn startup_seeding_runs_again_for(world: &ExampleDataSeedingWorld, seed_key: String) {
    world.run_seeding(&seed_key);
}

// -----------------------------------------------------------------------------
// Then Steps
// -----------------------------------------------------------------------------

#[then("the seeding result is {expected}")]
fn the_seeding_result_is(world: &ExampleDataSeedingWorld, expected: String) {
    if world.is_skipped() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
        return;
    }

    let result = world
        .last_result
        .get()
        .expect("seeding result should be set");

    match expected.trim_matches('"') {
        "applied" => match result {
            Ok(outcome) if outcome.result == backend::domain::ports::SeedingResult::Applied => {}
            Ok(outcome) => {
                panic!("expected Applied, got {:?}", outcome.result);
            }
            Err(err) => panic!("expected Applied, got error: {err}"),
        },
        "already seeded" => match result {
            Ok(outcome)
                if outcome.result == backend::domain::ports::SeedingResult::AlreadySeeded => {}
            Ok(outcome) => {
                panic!("expected AlreadySeeded, got {:?}", outcome.result);
            }
            Err(err) => panic!("expected AlreadySeeded, got error: {err}"),
        },
        other => panic!("unknown expected result: {other}"),
    }
}

/// Assert that the stored row count matches the expected value for a slot.
fn assert_count_stored(
    world: &ExampleDataSeedingWorld,
    expected_count: i64,
    count_slot: &Slot<i64>,
    error_msg: &str,
) {
    if world.is_skipped() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
        return;
    }

    world.record_table_counts();
    let actual_count = count_slot.get().expect(error_msg);
    assert_eq!(actual_count, expected_count);
}

#[then("{count} users are stored")]
fn users_are_stored(world: &ExampleDataSeedingWorld, count: i64) {
    assert_count_stored(
        world,
        count,
        &world.last_user_count,
        "user count should be recorded",
    );
}

#[then("{count} preferences are stored")]
fn preferences_are_stored(world: &ExampleDataSeedingWorld, count: i64) {
    assert_count_stored(
        world,
        count,
        &world.last_preferences_count,
        "preferences count should be recorded",
    );
}

#[then("a seeding error is returned")]
fn a_seeding_error_is_returned(world: &ExampleDataSeedingWorld) {
    if world.is_skipped() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
        return;
    }

    let result = world
        .last_result
        .get()
        .expect("seeding result should be set");
    assert!(result.is_err(), "expected error, got {result:?}");
}

// -----------------------------------------------------------------------------
// Scenario Bindings
// -----------------------------------------------------------------------------

#[scenario(
    path = "tests/features/example_data_seeding.feature",
    name = "First seed run applies example data"
)]
fn first_seed_run_applies_example_data(world: ExampleDataSeedingWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/example_data_seeding.feature",
    name = "Seed run is skipped when already applied"
)]
fn seed_run_is_skipped_when_already_applied(world: ExampleDataSeedingWorld) {
    let _ = world;
}

#[scenario(
    path = "tests/features/example_data_seeding.feature",
    name = "Missing seed returns an error"
)]
fn missing_seed_returns_error(world: ExampleDataSeedingWorld) {
    let _ = world;
}
