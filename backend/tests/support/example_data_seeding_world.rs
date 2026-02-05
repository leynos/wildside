//! Support types and step definitions for example data startup seeding scenarios.

use std::path::PathBuf;
use std::sync::Arc;

use backend::domain::ExampleDataSeedOutcome;
use backend::domain::ports::SeedingResult;
use backend::example_data::{ExampleDataSettings, seed_example_data_on_startup};
use backend::outbound::persistence::{DbPool, PoolConfig};
use cap_std::{ambient_authority, fs::Dir};
use diesel::QueryableByName;
use diesel::sql_query;
use diesel::sql_types::BigInt;
use diesel_async::RunQueryDsl;
use example_data::SeedRegistry;
use pg_embedded_setup_unpriv::TemporaryDatabase;
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, then, when};
use tokio::runtime::Runtime;
use uuid::Uuid;

use super::{handle_cluster_setup_failure, provision_template_database};
use crate::pg_embed::shared_cluster;

/// Wrapper for non-Clone runtime handle.
#[derive(Clone)]
struct RuntimeHandle(Arc<Runtime>);

/// Wrapper for the temporary database handle.
#[derive(Clone)]
struct DatabaseHandle(
    #[expect(dead_code, reason = "Retains the temporary database for each scenario")]
    Arc<TemporaryDatabase>,
);

/// Count row for raw SQL queries.
#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

/// World state for example data seeding BDD scenarios.
#[derive(Default, ScenarioState)]
pub struct ExampleDataSeedingWorld {
    runtime: Slot<RuntimeHandle>,
    pool: Slot<DbPool>,
    registry_path: Slot<PathBuf>,
    is_seeding_enabled: Slot<bool>,
    is_database_enabled: Slot<bool>,
    last_result: Slot<Result<Option<ExampleDataSeedOutcome>, String>>,
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

        self.runtime.set(RuntimeHandle(Arc::new(runtime)));
        self.pool.set(pool);
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
        let _registry = SeedRegistry::from_json(&registry_json).expect("registry should parse");
        let temp_dir = std::env::temp_dir();
        let file_name = format!("example-data-seeds-{}.json", Uuid::new_v4());
        let dir = Dir::open_ambient_dir(&temp_dir, ambient_authority()).expect("open temp dir");
        dir.write(&file_name, registry_json.as_bytes())
            .expect("write registry");
        let registry_path = temp_dir.join(&file_name);
        self.registry_path.set(registry_path);
    }

    fn execute_async<T>(&self, operation: impl FnOnce(&Runtime, &DbPool) -> T) -> Option<T> {
        if self.is_skipped() {
            return None;
        }

        let runtime = self.runtime.get().expect("runtime");
        let pool = self.pool.get().expect("pool");

        Some(operation(&runtime.0, &pool))
    }

    fn set_seeding_enabled(&self, enabled: bool) {
        self.is_seeding_enabled.set(enabled);
    }

    fn set_database_enabled(&self, enabled: bool) {
        self.is_database_enabled.set(enabled);
    }

    fn set_registry_path(&self, registry_path: PathBuf) {
        self.registry_path.set(registry_path);
    }

    fn is_seeding_enabled(&self) -> bool {
        self.is_seeding_enabled.get().unwrap_or(true)
    }

    fn is_database_enabled(&self) -> bool {
        self.is_database_enabled.get().unwrap_or(true)
    }

    fn build_settings(&self, seed_key: &str) -> ExampleDataSettings {
        ExampleDataSettings {
            is_enabled: self.is_seeding_enabled(),
            seed_name: Some(seed_key.to_owned()),
            count: None,
            registry_path: self.registry_path.get(),
        }
    }

    fn run_startup_seeding(&self, seed_key: &str) {
        let seed_key = seed_key.trim_matches('"');
        let settings = self.build_settings(seed_key);
        let is_database_enabled = self.is_database_enabled();
        if let Some(result) = self.execute_async(|runtime, pool| {
            let db_pool = is_database_enabled.then_some(pool);
            runtime
                .block_on(seed_example_data_on_startup(&settings, db_pool))
                .map_err(|error| error.to_string())
        }) {
            self.last_result.set(result);
        }
    }

    fn record_table_counts(&self) {
        if let Some((users, prefs)) = self.execute_async(|runtime, pool| {
            runtime.block_on(async {
                let users = count_table(pool, TableName::Users).await;
                let prefs = count_table(pool, TableName::UserPreferences).await;
                (users, prefs)
            })
        }) {
            self.last_user_count.set(users);
            self.last_preferences_count.set(prefs);
        }
    }
}

#[derive(Clone, Copy)]
enum TableName {
    Users,
    UserPreferences,
}

async fn count_table(pool: &DbPool, table: TableName) -> i64 {
    let mut conn = pool.get().await.expect("get connection");
    let query = match table {
        TableName::Users => "SELECT COUNT(*) AS count FROM users",
        TableName::UserPreferences => "SELECT COUNT(*) AS count FROM user_preferences",
    };
    let row: CountRow = sql_query(query)
        .get_result(&mut conn)
        .await
        .expect("count query");
    row.count
}

#[fixture]
pub fn world() -> ExampleDataSeedingWorld {
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

#[given("example data seeding is disabled")]
fn example_data_seeding_is_disabled(world: &ExampleDataSeedingWorld) {
    world.set_seeding_enabled(false);
}

#[given("the database is unavailable")]
fn database_is_unavailable(world: &ExampleDataSeedingWorld) {
    world.set_database_enabled(false);
}

#[given("an invalid registry path")]
fn an_invalid_registry_path(world: &ExampleDataSeedingWorld) {
    let registry_path =
        std::env::temp_dir().join(format!("missing-registry-{}.json", Uuid::new_v4()));
    world.set_registry_path(registry_path);
}

// -----------------------------------------------------------------------------
// When Steps
// -----------------------------------------------------------------------------

#[when("startup seeding runs for {seed_key}")]
fn startup_seeding_runs_for(world: &ExampleDataSeedingWorld, seed_key: String) {
    world.run_startup_seeding(&seed_key);
}

#[when("startup seeding runs again for {seed_key}")]
fn startup_seeding_runs_again_for(world: &ExampleDataSeedingWorld, seed_key: String) {
    world.run_startup_seeding(&seed_key);
}

// -----------------------------------------------------------------------------
// Then Steps
// -----------------------------------------------------------------------------

fn assert_seeding_result(
    result: &Result<Option<ExampleDataSeedOutcome>, String>,
    expected: SeedingResult,
) {
    match result {
        Ok(Some(outcome)) if outcome.result == expected => {}
        Ok(Some(outcome)) => {
            panic!("expected {expected:?}, got {:?}", outcome.result);
        }
        Ok(None) => panic!("expected {expected:?}, got skip"),
        Err(err) => panic!("expected {expected:?}, got error: {err}"),
    }
}

fn parse_expected_result(expected: &str) -> SeedingResult {
    match expected {
        "applied" => SeedingResult::Applied,
        "already seeded" => SeedingResult::AlreadySeeded,
        other => panic!("unknown expected result: {other}"),
    }
}

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

    let expected = parse_expected_result(expected.trim_matches('"'));
    assert_seeding_result(&result, expected);
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

#[then("startup seeding is skipped")]
fn startup_seeding_is_skipped(world: &ExampleDataSeedingWorld) {
    if world.is_skipped() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
        return;
    }

    let result = world
        .last_result
        .get()
        .expect("seeding result should be set");
    assert!(matches!(&result, Ok(None)), "expected skip, got {result:?}");
}
