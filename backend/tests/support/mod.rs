//! Shared helper utilities for backend integration tests.
//!
//! Integration tests compile as separate crates under `backend/tests/`, which
//! makes it awkward to share small helpers without copy/paste. This module
//! provides a small, dependency-free (relative to the test crate) home for
//! common test-only utilities.

pub mod atexit_cleanup;
mod cluster_skip;
pub mod embedded_postgres;
#[cfg(feature = "example-data")]
pub mod example_data_seeding_world;

/// Render a `postgres` error with enough detail to be useful in CI logs.
///
/// The `postgres::Error` `Display` implementation often collapses database
/// errors to a generic `db error`, which hides the message and SQLSTATE.
/// Prefer using `as_db_error()` when available so failures are actionable.
pub fn format_postgres_error(error: &postgres::Error) -> String {
    let Some(db_error) = error.as_db_error() else {
        return error.to_string();
    };

    let mut summary = format!(
        "postgres error {:?}: {}",
        db_error.code(),
        db_error.message()
    );

    if let Some(detail) = db_error.detail() {
        summary.push_str("; detail: ");
        summary.push_str(detail);
    }

    if let Some(hint) = db_error.hint() {
        summary.push_str("; hint: ");
        summary.push_str(hint);
    }

    if let Some(where_) = db_error.where_() {
        summary.push_str("; where: ");
        summary.push_str(where_);
    }

    summary
}

/// Seed a user and route fixture row pair for integration tests.
///
/// This centralizes common test setup so repository suites share the same
/// seeding contract and SQL shape.
// This helper is consumed by only a subset of integration-test crates.
#[allow(dead_code)]
pub fn seed_user_and_route(
    url: &str,
    user_id: &backend::domain::UserId,
    route_id: uuid::Uuid,
    display_name: &str,
) -> Result<(), String> {
    let mut client = postgres::Client::connect(url, postgres::NoTls)
        .map_err(|err| format_postgres_error(&err))?;
    let user_uuid = *user_id.as_uuid();

    client
        .execute(
            "INSERT INTO users (id, display_name) VALUES ($1, $2)",
            &[&user_uuid, &display_name],
        )
        .map_err(|err| format_postgres_error(&err))?;

    client
        .execute(
            concat!(
                "INSERT INTO routes (id, user_id, path, generation_params) ",
                "VALUES ($1, $2, '((0,0),(1,1))'::path, '{}'::jsonb)"
            ),
            &[&route_id, &user_uuid],
        )
        .map_err(|err| format_postgres_error(&err))?;

    Ok(())
}

/// Drop a table by name from a test database.
///
/// The identifier is escaped so helpers can safely accept test-provided table
/// names.
// This helper is consumed by only a subset of integration-test crates.
#[allow(dead_code)]
pub fn drop_table(url: &str, table_name: &str) -> Result<(), String> {
    let mut client = postgres::Client::connect(url, postgres::NoTls)
        .map_err(|err| format_postgres_error(&err))?;
    let escaped_name = table_name.replace('"', "\"\"");
    let sql = format!(r#"DROP TABLE IF EXISTS "{escaped_name}""#);
    client
        .batch_execute(sql.as_str())
        .map_err(|err| format_postgres_error(&err))
}

pub use cluster_skip::handle_cluster_setup_failure;
pub use embedded_postgres::provision_template_database;
