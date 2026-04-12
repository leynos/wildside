//! Database provisioning and seeding helpers for startup-mode composition
//! BDD tests.
//!
//! Extracted from `flow_support` to keep each module within the 400-line
//! limit.

use backend::outbound::persistence::{DbPool, PoolConfig};
use diesel_async::RunQueryDsl;
use pg_embedded_setup_unpriv::TemporaryDatabase;
use uuid::Uuid;

use super::support::atexit_cleanup::shared_cluster_handle;
use super::support::provision_template_database;

/// Database context for DB-present startup mode tests.
pub(crate) struct DbContext {
    pub(crate) database_url: String,
    pub(crate) pool: DbPool,
    pub(crate) _database: TemporaryDatabase,
}

/// Set up a DB context with embedded PostgreSQL.
pub(crate) fn setup_db_context(runtime: &tokio::runtime::Runtime) -> Result<DbContext, String> {
    let cluster = shared_cluster_handle().map_err(|error| error.to_string())?;
    let database = provision_template_database(cluster).map_err(|error| error.to_string())?;
    let database_url = database.url().to_owned();
    let pool = runtime
        .block_on(DbPool::new(
            PoolConfig::new(database_url.as_str())
                .with_max_size(2)
                .with_min_idle(Some(1)),
        ))
        .map_err(|error| error.to_string())?;
    Ok(DbContext {
        database_url,
        pool,
        _database: database,
    })
}

/// Seed a user into the DB for testing.
pub(crate) fn seed_user(
    pool: &DbPool,
    user_id: Uuid,
    display_name: &str,
    runtime: &tokio::runtime::Runtime,
) -> Result<(), String> {
    runtime.block_on(async {
        let mut conn = pool.get().await.map_err(|error| error.to_string())?;
        diesel::sql_query("INSERT INTO users (id, display_name) VALUES ($1, $2)")
            .bind::<diesel::sql_types::Uuid, _>(user_id)
            .bind::<diesel::sql_types::Text, _>(display_name)
            .execute(&mut conn)
            .await
            .map_err(|error| error.to_string())
            .map(|_| ())
    })
}

/// Seed a route into the DB for testing (walk sessions require a FK to routes).
pub(crate) fn seed_route(
    pool: &DbPool,
    route_id: Uuid,
    user_id: Uuid,
    runtime: &tokio::runtime::Runtime,
) -> Result<(), String> {
    runtime.block_on(async {
        let mut conn = pool.get().await.map_err(|error| error.to_string())?;
        diesel::sql_query(
            "INSERT INTO routes (id, user_id, path, generation_params) \
             VALUES ($1, $2, '((0,0),(1,1))'::path, '{}'::jsonb)",
        )
        .bind::<diesel::sql_types::Uuid, _>(route_id)
        .bind::<diesel::sql_types::Uuid, _>(user_id)
        .execute(&mut conn)
        .await
        .map_err(|error| error.to_string())
        .map(|_| ())
    })
}
