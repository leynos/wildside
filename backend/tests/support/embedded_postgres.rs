//! Shared embedded PostgreSQL helpers for integration tests.
//!
//! These helpers keep embedded PostgreSQL setup consistent across integration
//! test suites:
//!
//! - Database reset and creation use `postgres` to avoid Diesel transaction
//!   semantics interfering with `DROP DATABASE`.
//! - Schema setup runs embedded Diesel migrations so test schemas do not drift.
//! - Table teardown helpers provide a standard way to simulate schema loss.

use backend::domain::ports::UserPersistenceError;
use diesel::pg::PgConnection;
use diesel::Connection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use pg_embedded_setup_unpriv::TestCluster;
use postgres::{Client, NoTls};

use super::format_postgres_error;

/// Embedded migrations from the backend/migrations directory.
const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

/// Drops and recreates a database within the embedded cluster.
pub fn reset_database(cluster: &TestCluster, db_name: &str) -> Result<(), UserPersistenceError> {
    let admin_url = cluster.connection().database_url("postgres");
    let mut client = Client::connect(&admin_url, NoTls)
        .map_err(|err| UserPersistenceError::connection(format_postgres_error(&err)))?;

    // `DROP DATABASE` cannot run inside a transaction block. When multiple SQL
    // statements are sent in a single `batch_execute`, Postgres treats it as a
    // transaction block and rejects `DROP DATABASE`.
    client
        .batch_execute(&format!("DROP DATABASE IF EXISTS \"{db_name}\";"))
        .map_err(|err| UserPersistenceError::query(format_postgres_error(&err)))?;
    client
        .batch_execute(&format!("CREATE DATABASE \"{db_name}\";"))
        .map_err(|err| UserPersistenceError::query(format_postgres_error(&err)))?;
    Ok(())
}

/// Runs all pending Diesel migrations against the test database.
pub fn migrate_schema(url: &str) -> Result<(), UserPersistenceError> {
    let mut conn = PgConnection::establish(url)
        .map_err(|err| UserPersistenceError::connection(err.to_string()))?;
    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|err| UserPersistenceError::query(err.to_string()))?;
    Ok(())
}

/// Drops the `users` table from the given database URL.
pub fn drop_users_table(url: &str) -> Result<(), UserPersistenceError> {
    let mut client = Client::connect(url, NoTls)
        .map_err(|err| UserPersistenceError::connection(format_postgres_error(&err)))?;
    client
        .batch_execute("DROP TABLE IF EXISTS users;")
        .map_err(|err| UserPersistenceError::query(format_postgres_error(&err)))?;
    Ok(())
}
