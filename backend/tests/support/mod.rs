//! Shared helper utilities for backend integration tests.
//!
//! Integration tests compile as separate crates under `backend/tests/`, which
//! makes it awkward to share small helpers without copy/paste. This module
//! provides a small, dependency-free (relative to the test crate) home for
//! common test-only utilities.

pub mod atexit_cleanup;
// Note: these modules use #[allow(dead_code)] rather than #[expect(dead_code)]
// because each integration test file compiles support/mod.rs as its own crate.
// In crates that use a helper the lint is absent, so #[expect] would fail with
// "unfulfilled lint expectation". #[allow] is the only correct form here.
#[allow(
    dead_code,
    reason = "cluster skip helpers are only used by selected integration-test crates"
)]
mod cluster_skip;
pub mod embedded_postgres;
#[cfg(feature = "example-data")]
pub mod example_data_seeding_world;
#[allow(
    dead_code,
    reason = "shared profile/interests helpers are only used by selected integration-test crates"
)]
pub mod profile_interests;
#[allow(
    dead_code,
    reason = "shared Redis helpers are only used by selected integration-test crates"
)]
pub mod redis;
#[allow(
    dead_code,
    reason = "Redis skip helpers are only used by selected integration-test crates"
)]
mod redis_skip;
pub mod seed_helpers;

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

/// Drop a table by name from a test database.
///
/// The identifier is escaped so helpers can safely accept test-provided table
/// names.
///
/// # Examples
///
/// ```ignore
/// let url = "postgres://localhost/test";
/// let result = crate::support::drop_table(url, "offline_bundles");
/// assert!(result.is_ok());
/// ```
pub fn drop_table(url: &str, table_name: &str) -> Result<(), String> {
    let mut client = postgres::Client::connect(url, postgres::NoTls)
        .map_err(|err| format_postgres_error(&err))?;
    let escaped_name = table_name.replace('"', "\"\"");
    let sql = format!(r#"DROP TABLE IF EXISTS "{escaped_name}""#);
    client
        .batch_execute(sql.as_str())
        .map_err(|err| format_postgres_error(&err))
}

// Anchor shared helper reachability across independent integration-test crates.
const _: fn(&str, &str) -> Result<(), String> = drop_table;

// Re-export skip helpers for integration test crates.
// Not marked with #[expect(unused_imports)] because usage varies across
// integration-test crates and unfulfilled expectations would break builds.
#[allow(
    unused_imports,
    reason = "Cluster skip helpers are only used by selected integration-test crates"
)]
pub use cluster_skip::handle_cluster_setup_failure;
pub use embedded_postgres::provision_template_database;
#[allow(
    unused_imports,
    reason = "Redis skip helpers are only used by selected integration-test crates"
)]
pub use redis_skip::should_skip_redis_tests;
// Re-exported for crates that import from `support` directly.
#[expect(
    unused_imports,
    reason = "some integration test crates import support helpers through this facade"
)]
pub use seed_helpers::*;
