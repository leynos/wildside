//! Shared helper utilities for backend integration tests.
//!
//! Integration tests compile as separate crates under `backend/tests/`, which
//! makes it awkward to share small helpers without copy/paste. This module
//! provides a small, dependency-free (relative to the test crate) home for
//! common test-only utilities.

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
