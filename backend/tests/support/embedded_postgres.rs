//! Shared embedded PostgreSQL helpers for integration tests.
//!
//! These helpers keep embedded PostgreSQL setup consistent across integration
//! test suites:
//!
//! - Database reset and creation use `postgres` to avoid Diesel transaction
//!   semantics interfering with `DROP DATABASE`.
//! - Schema setup runs embedded Diesel migrations so test schemas do not drift.
//! - Table teardown helpers provide a standard way to simulate schema loss.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use backend::domain::ports::UserPersistenceError;
use diesel::Connection;
use diesel::pg::PgConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use pg_embedded_setup_unpriv::test_support::hash_directory;
use pg_embedded_setup_unpriv::{ClusterHandle, TemporaryDatabase};
use postgres::{Client, NoTls};
use uuid::Uuid;

use super::format_postgres_error;

/// Embedded migrations from the backend/migrations directory.
const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

static TEMPLATE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

const TEMPLATE_NAME_PREFIX: &str = "backend_template";
const TEMPLATE_PROVISION_RETRIES: usize = 5;
const TEMPLATE_PROVISION_RETRY_DELAY: Duration = Duration::from_millis(500);

fn migrations_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations")
}

fn template_database_name() -> Result<String, UserPersistenceError> {
    let hash = hash_directory(migrations_dir())
        .map_err(|err| UserPersistenceError::query(format!("hash migrations: {err}")))?;
    let short_hash = hash.get(..8).unwrap_or(&hash);
    Ok(format!("{TEMPLATE_NAME_PREFIX}_{short_hash}"))
}

fn new_test_database_name() -> String {
    format!("test_{}", Uuid::new_v4())
}

/// Attempts one template-clone provisioning pass for a test database.
///
/// Uses `attempt` (typically in the `1..=TEMPLATE_PROVISION_RETRIES` range) to
/// annotate retry-aware `UserPersistenceError` messages. Returns
/// `Ok(TemporaryDatabase)` when both template resolution
/// (`ensure_template_database`) and `ClusterHandle::temporary_database_from_template`
/// succeed, and returns `Err(UserPersistenceError)` when either step fails.
///
/// # Examples
///
/// ```text
/// attempt = 1
/// provision_template_database_attempt(&cluster, attempt)
///   -> Ok(TemporaryDatabase { name: "test_1234...", .. })
///
/// attempt = 1
/// provision_template_database_attempt(&cluster, attempt)
///   -> Err(UserPersistenceError::query(
///        "template check: attempt 1/5: ..."
///      ))
/// ```
fn provision_template_database_attempt(
    cluster: &ClusterHandle,
    attempt: usize,
) -> Result<TemporaryDatabase, UserPersistenceError> {
    let template_name = ensure_template_database(cluster).map_err(|error| {
        UserPersistenceError::query(format!(
            "template check: attempt {attempt}/{TEMPLATE_PROVISION_RETRIES}: {error}"
        ))
    })?;
    let db_name = new_test_database_name();
    cluster
        .temporary_database_from_template(db_name.as_str(), template_name.as_str())
        .map_err(|error| {
            UserPersistenceError::query(format!(
                "create database from template: attempt {attempt}/{TEMPLATE_PROVISION_RETRIES}: {error:?}"
            ))
        })
}

/// Creates or reuses a template database with the latest migrations applied.
fn ensure_template_database(cluster: &ClusterHandle) -> Result<String, UserPersistenceError> {
    let template_name = template_database_name()?;
    let _lock = TEMPLATE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|err| err.into_inner());

    let exists = cluster
        .database_exists(template_name.as_str())
        .map_err(|err| UserPersistenceError::query(format!("template check: {err:?}")))?;

    if !exists {
        cluster
            .create_database(template_name.as_str())
            .map_err(|err| UserPersistenceError::query(format!("create template: {err:?}")))?;

        let url = cluster.connection().database_url(&template_name);
        migrate_schema(&url)?;
    }

    Ok(template_name)
}

/// Provisions a temporary database cloned from the migration template.
pub fn provision_template_database(
    cluster: &ClusterHandle,
) -> Result<TemporaryDatabase, UserPersistenceError> {
    let mut last_error = None;
    for attempt in 1..=TEMPLATE_PROVISION_RETRIES {
        match provision_template_database_attempt(cluster, attempt) {
            Ok(database) => return Ok(database),
            Err(error) => last_error = Some(error),
        };
        if attempt < TEMPLATE_PROVISION_RETRIES {
            std::thread::sleep(TEMPLATE_PROVISION_RETRY_DELAY);
        }
    }

    Err(last_error.unwrap_or_else(|| {
        UserPersistenceError::query("create database from template: exhausted retries")
    }))
}

/// Runs all pending Diesel migrations against the test database.
pub fn migrate_schema(url: &str) -> Result<(), UserPersistenceError> {
    let mut conn = PgConnection::establish(url)
        .map_err(|err| UserPersistenceError::connection(format!("{err:?}")))?;
    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|err| UserPersistenceError::query(format!("migration: {err:?}")))?;
    Ok(())
}

/// Drops the `users` table from the given database URL.
///
/// Uses CASCADE to also drop tables that have foreign key dependencies on users
/// (e.g., routes, user_preferences, route_notes, route_progress).
// Only select integration suites exercise user table loss scenarios.
pub fn drop_users_table(url: &str) -> Result<(), UserPersistenceError> {
    let mut client = Client::connect(url, NoTls)
        .map_err(|err| UserPersistenceError::connection(format_postgres_error(&err)))?;
    client
        .batch_execute("DROP TABLE IF EXISTS users CASCADE;")
        .map_err(|err| UserPersistenceError::query(format_postgres_error(&err)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Linkage checks for embedded postgres helpers.

    use super::*;

    #[test]
    fn drop_users_table_is_linked() {
        let _ = drop_users_table as fn(&str) -> Result<(), UserPersistenceError>;
    }
}
