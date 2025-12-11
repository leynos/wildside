//! Integration tests for `DieselUserRepository` against embedded PostgreSQL.
//!
//! These tests verify that the Diesel-backed user repository correctly
//! implements the `UserRepository` port contract against a real PostgreSQL
//! database. Tests use `pg-embedded-setup-unpriv` for isolated database
//! instances.

use std::sync::{Arc, Mutex};

use actix_rt::System;
use backend::domain::ports::{UserPersistenceError, UserRepository};
use backend::domain::{DisplayName, User, UserId};
use backend::outbound::persistence::{DbPool, DieselUserRepository, PoolConfig};
use pg_embedded_setup_unpriv::TestCluster;
use postgres::{Client, NoTls};
use rstest::{fixture, rstest};
use rstest_bdd_macros::{given, then, when};

const TEST_DB: &str = "diesel_user_repo_test";

// -----------------------------------------------------------------------------
// Fixtures
// -----------------------------------------------------------------------------

#[fixture]
fn sample_user_id() -> String {
    "11111111-1111-1111-1111-111111111111".to_owned()
}

#[fixture]
fn sample_display_name() -> DisplayName {
    DisplayName::new("Diesel Test User").expect("valid display name")
}

#[fixture]
fn sample_user(sample_user_id: String, sample_display_name: DisplayName) -> User {
    User::try_from_strings(sample_user_id, sample_display_name.as_ref())
        .expect("fixture user is valid")
}

// -----------------------------------------------------------------------------
// Test Context
// -----------------------------------------------------------------------------

struct TestContext {
    _cluster: TestCluster,
    repository: DieselUserRepository,
    database_url: String,
    last_upsert_error: Option<UserPersistenceError>,
    last_fetch_result: Option<Result<Option<User>, UserPersistenceError>>,
    persisted_user: Option<User>,
}

type SharedContext = Arc<Mutex<TestContext>>;

fn setup_test_context() -> Result<TestContext, String> {
    let cluster = TestCluster::new().map_err(|err| err.to_string())?;

    // Create the test database.
    reset_database(&cluster).map_err(|err| err.to_string())?;

    let database_url = cluster.connection().database_url(TEST_DB);

    // Run schema migration.
    migrate_schema(&database_url).map_err(|err| err.to_string())?;

    // Create the connection pool and repository.
    let config = PoolConfig::new(&database_url)
        .with_max_size(2)
        .with_min_idle(Some(1));

    let pool = System::new()
        .block_on(async { DbPool::new(config).await })
        .map_err(|err| err.to_string())?;

    let repository = DieselUserRepository::new(pool);

    Ok(TestContext {
        _cluster: cluster,
        repository,
        database_url,
        last_upsert_error: None,
        last_fetch_result: None,
        persisted_user: None,
    })
}

#[fixture]
fn diesel_world() -> Option<SharedContext> {
    match setup_test_context() {
        Ok(ctx) => Some(Arc::new(Mutex::new(ctx))),
        Err(reason) => {
            eprintln!("SKIP-TEST-CLUSTER: {reason}");
            None
        }
    }
}

// -----------------------------------------------------------------------------
// BDD Step Definitions
// -----------------------------------------------------------------------------

#[given("a Diesel-backed user repository")]
fn a_diesel_backed_user_repository(_world: SharedContext) {
    // Context already initialised with repository.
}

#[when("the repository upserts the user")]
fn the_repository_upserts_the_user(world: SharedContext, user: User) {
    let repo = {
        let ctx = world.lock().expect("context lock");
        ctx.repository.clone()
    };
    let stored_user = user.clone();
    let result = System::new().block_on(async move { repo.upsert(&user).await });
    let mut ctx = world.lock().expect("context lock");
    match result {
        Ok(()) => {
            ctx.last_upsert_error = None;
            ctx.persisted_user = Some(stored_user);
        }
        Err(err) => {
            ctx.last_upsert_error = Some(err);
        }
    }
}

#[when("the repository fetches the user by id")]
fn the_repository_fetches_the_user_by_id(world: SharedContext) {
    let (repo, user_id) = {
        let ctx = world.lock().expect("context lock");
        let id = ctx
            .persisted_user
            .as_ref()
            .expect("user should have been persisted")
            .id()
            .clone();
        (ctx.repository.clone(), id)
    };
    let result = System::new().block_on(async move { repo.find_by_id(&user_id).await });
    let mut ctx = world.lock().expect("context lock");
    ctx.last_fetch_result = Some(result);
}

#[when("the users table is dropped")]
fn the_users_table_is_dropped(world: SharedContext) {
    let url = {
        let ctx = world.lock().expect("context lock");
        ctx.database_url.clone()
    };
    drop_users_table(&url).expect("drop succeeds");
}

#[then("the stored user is returned")]
fn the_stored_user_is_returned(world: SharedContext, expected: User) {
    let ctx = world.lock().expect("context lock");
    let result = ctx.last_fetch_result.as_ref().expect("fetch was executed");
    match result {
        Ok(Some(user)) => assert_eq!(user, &expected),
        Ok(None) => panic!("expected user but got None"),
        Err(err) => panic!("expected user but got error: {err}"),
    }
}

#[then("persistence fails with a query error")]
fn persistence_fails_with_a_query_error(world: SharedContext) {
    let ctx = world.lock().expect("context lock");
    assert!(
        matches!(
            ctx.last_upsert_error,
            Some(UserPersistenceError::Query { .. })
        ),
        "expected Query error, got: {:?}",
        ctx.last_upsert_error
    );
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[rstest]
fn diesel_user_repository_round_trip(diesel_world: Option<SharedContext>, sample_user: User) {
    let Some(world) = diesel_world else {
        eprintln!("SKIP-TEST-CLUSTER: diesel_user_repository_round_trip skipped");
        return;
    };

    a_diesel_backed_user_repository(world.clone());
    the_repository_upserts_the_user(world.clone(), sample_user.clone());
    the_repository_fetches_the_user_by_id(world.clone());
    the_stored_user_is_returned(world, sample_user);
}

#[rstest]
fn diesel_upsert_updates_existing_user(diesel_world: Option<SharedContext>) {
    let Some(world) = diesel_world else {
        eprintln!("SKIP-TEST-CLUSTER: diesel_upsert_updates_existing_user skipped");
        return;
    };

    let user_v1 = User::try_from_strings("22222222-2222-2222-2222-222222222222", "Original Name")
        .expect("valid user");

    let user_v2 = User::try_from_strings("22222222-2222-2222-2222-222222222222", "Updated Name")
        .expect("valid user");

    let repo = {
        let ctx = world.lock().expect("context lock");
        ctx.repository.clone()
    };

    System::new().block_on(async {
        repo.upsert(&user_v1).await.expect("first upsert");
        repo.upsert(&user_v2).await.expect("second upsert");

        let fetched = repo.find_by_id(user_v2.id()).await.expect("fetch succeeds");
        assert_eq!(
            fetched.expect("user exists").display_name().as_ref(),
            "Updated Name"
        );
    });
}

#[rstest]
fn diesel_find_nonexistent_returns_none(diesel_world: Option<SharedContext>) {
    let Some(world) = diesel_world else {
        eprintln!("SKIP-TEST-CLUSTER: diesel_find_nonexistent_returns_none skipped");
        return;
    };

    let nonexistent_id = UserId::new("99999999-9999-9999-9999-999999999999").expect("valid UUID");

    let repo = {
        let ctx = world.lock().expect("context lock");
        ctx.repository.clone()
    };

    let result = System::new().block_on(async { repo.find_by_id(&nonexistent_id).await });
    assert!(
        result.expect("query succeeds").is_none(),
        "nonexistent user should return None"
    );
}

#[rstest]
fn diesel_reports_errors_when_schema_missing(
    diesel_world: Option<SharedContext>,
    sample_user: User,
) {
    let Some(world) = diesel_world else {
        eprintln!("SKIP-TEST-CLUSTER: diesel_reports_errors_when_schema_missing skipped");
        return;
    };

    a_diesel_backed_user_repository(world.clone());
    the_users_table_is_dropped(world.clone());
    the_repository_upserts_the_user(world.clone(), sample_user);
    persistence_fails_with_a_query_error(world);
}

// -----------------------------------------------------------------------------
// Database Helpers
// -----------------------------------------------------------------------------

/// Path to the users table migration, relative to the backend crate root.
const USERS_MIGRATION_UP: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/migrations/2025-12-10-000000_create_users/up.sql"
);

fn reset_database(cluster: &TestCluster) -> Result<(), UserPersistenceError> {
    let admin_url = cluster.connection().database_url("postgres");
    let mut client = Client::connect(&admin_url, NoTls)
        .map_err(|err| UserPersistenceError::connection(err.to_string()))?;
    client
        .batch_execute(&format!(
            "DROP DATABASE IF EXISTS \"{TEST_DB}\"; CREATE DATABASE \"{TEST_DB}\";"
        ))
        .map_err(|err| UserPersistenceError::query(err.to_string()))?;
    Ok(())
}

/// Run the actual Diesel migration to create the users table.
///
/// This ensures the test schema stays in sync with the real migration rather
/// than hand-coding the DDL and risking drift.
fn migrate_schema(url: &str) -> Result<(), UserPersistenceError> {
    let migration_sql = std::fs::read_to_string(USERS_MIGRATION_UP)
        .map_err(|err| UserPersistenceError::query(format!("failed to read migration: {err}")))?;

    let mut client = Client::connect(url, NoTls)
        .map_err(|err| UserPersistenceError::connection(err.to_string()))?;
    client
        .batch_execute(&migration_sql)
        .map_err(|err| UserPersistenceError::query(err.to_string()))?;
    Ok(())
}

fn drop_users_table(url: &str) -> Result<(), UserPersistenceError> {
    let mut client = Client::connect(url, NoTls)
        .map_err(|err| UserPersistenceError::connection(err.to_string()))?;
    client
        .batch_execute("DROP TABLE IF EXISTS users;")
        .map_err(|err| UserPersistenceError::query(err.to_string()))?;
    Ok(())
}
