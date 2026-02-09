//! Integration tests for `DieselUserRepository` against embedded PostgreSQL.
//!
//! These tests verify that the Diesel-backed user repository correctly
//! implements the `UserRepository` port contract against a real PostgreSQL
//! database. Tests use `pg-embedded-setup-unpriv` for isolated database
//! instances.
//!
//! # Runtime Strategy
//!
//! `rstest-bdd` v0.5.0 supports async step definitions, but this suite keeps
//! synchronous steps and reuses a shared Tokio runtime in the test context.
//! This keeps database operations deterministic and avoids recreating a runtime
//! for each step.
use std::sync::{Arc, Mutex};

use backend::domain::ports::{UserPersistenceError, UserRepository};
use backend::domain::{DisplayName, User, UserId};
use backend::outbound::persistence::{DbPool, DieselUserRepository, PoolConfig};
use pg_embedded_setup_unpriv::TemporaryDatabase;
use rstest::{fixture, rstest};
use rstest_bdd_macros::{given, then, when};
use tokio::runtime::Runtime;

#[path = "support/pg_embed.rs"]
mod pg_embed;

mod support;

use pg_embed::shared_cluster;
use support::embedded_postgres::drop_users_table;
use support::{handle_cluster_setup_failure, provision_template_database};

// -----------------------------------------------------------------------------
// Fixtures
// -----------------------------------------------------------------------------

#[fixture]
fn sample_user_id() -> UserId {
    UserId::new("11111111-1111-1111-1111-111111111111").expect("fixture user id is valid")
}

#[fixture]
fn sample_display_name() -> DisplayName {
    DisplayName::new("Diesel Test User").expect("valid display name")
}

#[fixture]
fn sample_user(sample_user_id: UserId, sample_display_name: DisplayName) -> User {
    User::new(sample_user_id, sample_display_name)
}

// -----------------------------------------------------------------------------
// Test Context
// -----------------------------------------------------------------------------

struct TestContext {
    /// Tokio runtime reused for all async operations in this test.
    runtime: Runtime,
    repository: DieselUserRepository,
    database_url: String,
    last_upsert_error: Option<UserPersistenceError>,
    last_fetch_result: Option<Result<Option<User>, UserPersistenceError>>,
    persisted_user: Option<User>,
    _database: TemporaryDatabase,
}

type SharedContext = Arc<Mutex<TestContext>>;

/// Extracts values from the locked context, executes an async operation,
/// and optionally updates the context with results.
fn with_context_async<F, R, U>(
    world: &SharedContext,
    extract: impl FnOnce(&TestContext) -> F,
    operation: impl FnOnce(DieselUserRepository, F) -> R,
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

    let repository = DieselUserRepository::new(pool);

    Ok(TestContext {
        runtime,
        repository,
        database_url,
        last_upsert_error: None,
        last_fetch_result: None,
        persisted_user: None,
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

#[given("a Diesel-backed user repository")]
fn a_diesel_backed_user_repository(_world: SharedContext) {}

#[when("the repository upserts the user")]
fn the_repository_upserts_the_user(world: SharedContext, user: User) {
    let stored_user = user.clone();
    with_context_async(
        &world,
        |_| user,
        |repo, user| async move { repo.upsert(&user).await },
        |ctx, result| match result {
            Ok(()) => {
                ctx.last_upsert_error = None;
                ctx.persisted_user = Some(stored_user);
            }
            Err(err) => {
                ctx.last_upsert_error = Some(err);
            }
        },
    );
}

#[when("the repository fetches the user by id")]
fn the_repository_fetches_the_user_by_id(world: SharedContext) {
    with_context_async(
        &world,
        |ctx| {
            ctx.persisted_user
                .as_ref()
                .expect("user should have been persisted")
                .id()
                .clone()
        },
        |repo, user_id| async move { repo.find_by_id(&user_id).await },
        |ctx, result| {
            ctx.last_fetch_result = Some(result);
        },
    );
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
        Ok(None) => panic!(
            "expected user but got None; last_upsert_error: {:?}",
            ctx.last_upsert_error
        ),
        Err(err) => panic!(
            "expected user but got error: {err}; last_upsert_error: {:?}",
            ctx.last_upsert_error
        ),
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

    with_context_async(
        &world,
        |_| (user_v1, user_v2),
        |repo, (user_v1, user_v2)| async move {
            repo.upsert(&user_v1).await.expect("first upsert");
            repo.upsert(&user_v2).await.expect("second upsert");

            let fetched = repo.find_by_id(user_v2.id()).await.expect("fetch succeeds");
            assert_eq!(
                fetched.expect("user exists").display_name().as_ref(),
                "Updated Name"
            );
        },
        |_, _| {},
    );
}

#[rstest]
fn diesel_find_nonexistent_returns_none(diesel_world: Option<SharedContext>) {
    let Some(world) = diesel_world else {
        eprintln!("SKIP-TEST-CLUSTER: diesel_find_nonexistent_returns_none skipped");
        return;
    };

    let nonexistent_id = UserId::new("99999999-9999-9999-9999-999999999999").expect("valid UUID");

    let mut result = None;
    with_context_async(
        &world,
        |_| nonexistent_id,
        |repo, user_id| async move { repo.find_by_id(&user_id).await },
        |_, fetched| {
            result = Some(fetched);
        },
    );

    let result = result.expect("find_by_id should execute");
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
