//! Behavioural tests for the new domain ports backed by PostgreSQL.

use std::sync::{Arc, Mutex};

use actix_rt::System;
use backend::domain::ports::{UserPersistenceError, UserRepository};
use backend::domain::{DisplayName, User, UserId};
use pg_embedded_setup_unpriv::test_support::test_cluster;
use pg_embedded_setup_unpriv::TestCluster;
use postgres::{Client, NoTls};
use rstest::{fixture, rstest};
use rstest_bdd_macros::{given, then, when};
use uuid::Uuid;

const CONTRACT_DB: &str = "ports_contract";

#[fixture]
fn sample_id() -> String {
    "11111111-1111-1111-1111-111111111111".to_owned()
}

#[fixture]
fn sample_display_name() -> DisplayName {
    DisplayName::new("Route Ports User").expect("valid display name")
}

#[fixture]
fn sample_user(sample_id: String, sample_display_name: DisplayName) -> User {
    User::try_from_strings(sample_id, sample_display_name.as_ref()).expect("fixture user is valid")
}

#[derive(Clone)]
struct PgUserRepository {
    client: Arc<Mutex<Client>>,
}

impl PgUserRepository {
    fn connect(url: &str) -> Result<Self, UserPersistenceError> {
        let client = Client::connect(url, NoTls)
            .map_err(|err| UserPersistenceError::connection(err.to_string()))?;
        Ok(Self {
            client: Arc::new(Mutex::new(client)),
        })
    }
}

#[async_trait::async_trait]
impl UserRepository for PgUserRepository {
    async fn upsert(&self, user: &User) -> Result<(), UserPersistenceError> {
        let mut guard = self.client.lock().expect("pg client poisoned");
        let id = user.id().as_uuid();
        let display = user.display_name().as_ref();
        guard
            .execute(
                "INSERT INTO users (id, display_name) VALUES ($1, $2)
                 ON CONFLICT (id) DO UPDATE SET display_name = excluded.display_name",
                &[id, &display],
            )
            .map(|_| ())
            .map_err(|err| UserPersistenceError::query(err.to_string()))
    }

    async fn find_by_id(&self, id: &UserId) -> Result<Option<User>, UserPersistenceError> {
        let mut guard = self.client.lock().expect("pg client poisoned");
        let result = guard
            .query_opt(
                "SELECT id, display_name FROM users WHERE id = $1",
                &[id.as_uuid()],
            )
            .map_err(|err| UserPersistenceError::query(err.to_string()))?;

        if let Some(row) = result {
            let id: Uuid = row.get(0);
            let display: String = row.get(1);
            let user = User::try_from_strings(id.to_string(), display)
                .map_err(|err| UserPersistenceError::query(err.to_string()))?;
            Ok(Some(user))
        } else {
            Ok(None)
        }
    }
}

struct RepoContext {
    #[allow(dead_code)]
    cluster: TestCluster,
    repository: PgUserRepository,
    database_url: String,
    last_write_error: Option<UserPersistenceError>,
    last_fetch_value: Option<Option<User>>,
    last_fetch_error: Option<UserPersistenceError>,
    persisted_user: Option<User>,
}

type SharedContext = Arc<Mutex<RepoContext>>;

fn init_repo_context() -> Result<RepoContext, UserPersistenceError> {
    let cluster = test_cluster();
    reset_database(&cluster)?;
    let database_url = cluster.connection().database_url(CONTRACT_DB);
    migrate_schema(&database_url)?;
    let repository = PgUserRepository::connect(&database_url)?;
    Ok(RepoContext {
        cluster,
        repository,
        database_url,
        last_write_error: None,
        last_fetch_value: None,
        last_fetch_error: None,
        persisted_user: None,
    })
}

#[fixture]
fn repo_world() -> SharedContext {
    let context = init_repo_context().expect("context initialises");
    Arc::new(Mutex::new(context))
}

#[given("a postgres-backed user repository")]
fn a_postgres_backed_user_repository(_repo_world: SharedContext) {}

#[when("the repository upserts the user")]
fn the_repository_upserts_the_user(repo_world: SharedContext, user: User) {
    let repo = {
        let ctx = repo_world.lock().expect("context lock");
        ctx.repository.clone()
    };
    let stored_user = user.clone();
    let result = System::new().block_on(async move { repo.upsert(&user).await });
    let mut ctx = repo_world.lock().expect("context lock");
    match result {
        Ok(()) => {
            ctx.last_write_error = None;
            ctx.persisted_user = Some(stored_user);
        }
        Err(err) => {
            ctx.last_write_error = Some(err);
        }
    }
}

#[when("the repository fetches the user by id")]
fn the_repository_fetches_the_user(repo_world: SharedContext) {
    let (repo, user_id) = {
        let ctx = repo_world.lock().expect("context lock");
        let id = ctx
            .persisted_user
            .as_ref()
            .map(|user| user.id().clone())
            .expect("user should have been persisted");
        (ctx.repository.clone(), id)
    };
    let result = System::new().block_on(async move { repo.find_by_id(&user_id).await });
    let mut ctx = repo_world.lock().expect("context lock");
    match result {
        Ok(value) => {
            ctx.last_fetch_value = Some(value);
            ctx.last_fetch_error = None;
        }
        Err(err) => {
            ctx.last_fetch_value = None;
            ctx.last_fetch_error = Some(err);
        }
    }
}

#[when("the users table is dropped")]
fn the_users_table_is_dropped(repo_world: SharedContext) {
    let url = {
        let ctx = repo_world.lock().expect("context lock");
        ctx.database_url.clone()
    };
    drop_users_table(&url).expect("drop succeeds");
}

#[then("the stored user is returned")]
fn the_stored_user_is_returned(repo_world: SharedContext, expected: User) {
    let ctx = repo_world.lock().expect("context lock");
    assert!(
        ctx.last_fetch_error.is_none(),
        "fetch error: {:?}",
        ctx.last_fetch_error
    );
    let fetched = ctx
        .last_fetch_value
        .as_ref()
        .expect("fetch executed")
        .clone();
    assert_eq!(fetched, Some(expected));
}

#[then("persistence fails with a query error")]
fn persistence_fails_with_a_query_error(repo_world: SharedContext) {
    let ctx = repo_world.lock().expect("context lock");
    assert!(matches!(
        ctx.last_write_error,
        Some(UserPersistenceError::Query { .. })
    ));
}

#[rstest]
fn user_repository_round_trip(repo_world: SharedContext, sample_user: User) {
    a_postgres_backed_user_repository(repo_world.clone());
    the_repository_upserts_the_user(repo_world.clone(), sample_user.clone());
    the_repository_fetches_the_user(repo_world.clone());
    the_stored_user_is_returned(repo_world, sample_user);
}

#[rstest]
fn user_repository_reports_errors_when_schema_missing(
    repo_world: SharedContext,
    sample_user: User,
) {
    a_postgres_backed_user_repository(repo_world.clone());
    the_users_table_is_dropped(repo_world.clone());
    the_repository_upserts_the_user(repo_world.clone(), sample_user);
    persistence_fails_with_a_query_error(repo_world);
}

fn reset_database(cluster: &TestCluster) -> Result<(), UserPersistenceError> {
    let admin_url = cluster.connection().database_url("postgres");
    let mut client = Client::connect(&admin_url, NoTls)
        .map_err(|err| UserPersistenceError::connection(err.to_string()))?;
    client
        .batch_execute(&format!(
            "DROP DATABASE IF EXISTS \"{CONTRACT_DB}\"; CREATE DATABASE \"{CONTRACT_DB}\";"
        ))
        .map_err(|err| UserPersistenceError::query(err.to_string()))?;
    Ok(())
}

fn migrate_schema(url: &str) -> Result<(), UserPersistenceError> {
    let mut client = Client::connect(url, NoTls)
        .map_err(|err| UserPersistenceError::connection(err.to_string()))?;
    client
        .batch_execute(
            "CREATE TABLE IF NOT EXISTS users (
                id UUID PRIMARY KEY,
                display_name TEXT NOT NULL
            );",
        )
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
