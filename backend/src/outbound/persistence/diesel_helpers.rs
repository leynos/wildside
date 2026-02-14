//! Shared helpers and macros for Diesel repository implementations.
//!
//! This module provides common utilities for database access including:
//! - Error mapping from Diesel errors to domain errors
//! - Revision casting between database and domain types
//! - Traits and helpers for optimistic concurrency control
//! - Declarative macros for common query patterns

use tracing::{debug, warn};

use crate::domain::ports::RouteAnnotationRepositoryError;

use super::pool::PoolError;

/// Extract a readable message from a pool error.
pub fn map_pool_error_message(error: PoolError) -> String {
    match error {
        PoolError::Checkout { message } | PoolError::Build { message } => message,
    }
}

/// Extract a readable message from a Diesel error and emit debug context.
pub fn map_diesel_error_message(error: diesel::result::Error, operation: &str) -> String {
    let error_message = error.to_string();
    debug!(%error_message, %operation, "diesel operation failed");
    error_message
}

/// Map pool errors to domain route annotation repository errors.
pub fn map_pool_error(error: PoolError) -> RouteAnnotationRepositoryError {
    match error {
        PoolError::Checkout { message } | PoolError::Build { message } => {
            RouteAnnotationRepositoryError::connection(message)
        }
    }
}

/// Check if message indicates a foreign key constraint violation.
fn is_foreign_key_message(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("foreign key") || lower.contains("violates foreign key constraint")
}

/// Map foreign key violation to appropriate domain error.
///
/// Identifies route FK violations by inspecting the constraint name (when
/// available) and the error message for route-id-related constraints.
/// Unrecognised FK violations are logged for monitoring.
fn map_foreign_key_violation(
    message: &str,
    constraint_name: Option<&str>,
) -> RouteAnnotationRepositoryError {
    let lower = message.to_lowercase();
    let is_fk_message = is_foreign_key_message(message);
    let constraint_lower = constraint_name.map(str::to_lowercase);
    let references_route_fk = constraint_lower
        .as_deref()
        .map(|name| name.contains("route_id_fkey") || name.contains("routes"))
        .unwrap_or(false)
        || lower.contains("route_id_fkey")
        || (is_fk_message && lower.contains("routes"));

    if references_route_fk {
        RouteAnnotationRepositoryError::route_not_found("referenced route".to_string())
    } else {
        // Log unrecognised FK violations for monitoring; may indicate new FK
        // constraints that need specific handling.
        warn!(
            message,
            constraint_name = ?constraint_name,
            "unrecognised foreign key violation - may need specific error mapping"
        );
        RouteAnnotationRepositoryError::query("foreign key violation")
    }
}

/// Map Diesel errors to domain route annotation repository errors.
pub fn map_diesel_error(error: diesel::result::Error) -> RouteAnnotationRepositoryError {
    use diesel::result::{DatabaseErrorKind, Error as DieselError};

    match &error {
        DieselError::DatabaseError(kind, info) => {
            debug!(?kind, message = info.message(), "diesel operation failed");
        }
        _ => debug!(
            error_type = %std::any::type_name_of_val(&error),
            "diesel operation failed"
        ),
    }

    match error {
        DieselError::NotFound => RouteAnnotationRepositoryError::query("record not found"),
        DieselError::QueryBuilderError(_) => {
            RouteAnnotationRepositoryError::query("database query error")
        }
        DieselError::DatabaseError(kind, info) => match kind {
            DatabaseErrorKind::ForeignKeyViolation => {
                map_foreign_key_violation(info.message(), info.constraint_name())
            }
            DatabaseErrorKind::ClosedConnection => {
                RouteAnnotationRepositoryError::connection("database connection error")
            }
            _ => RouteAnnotationRepositoryError::query("database error"),
        },
        _ => RouteAnnotationRepositoryError::query("database error"),
    }
}

/// Cast database revision (i32) to domain revision (u32).
///
/// Database stores revisions as `i32` but domain uses `u32`. Revisions are
/// always non-negative in practice, enforced by database constraints.
#[expect(
    clippy::cast_sign_loss,
    reason = "revision is always non-negative in database"
)]
pub fn cast_revision(revision: i32) -> u32 {
    revision as u32
}

/// Cast domain revision (u32) to database revision (i32).
#[expect(
    clippy::cast_possible_wrap,
    reason = "revision values are always small positive integers"
)]
pub fn cast_revision_for_db(revision: u32) -> i32 {
    revision as i32
}

/// Trait for database rows that have a revision field.
pub trait HasRevision {
    /// Get the revision as a u32.
    fn revision(&self) -> u32;
}

/// Result of an optimistic update operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateResult {
    /// Update succeeded (one or more rows affected).
    Success,
    /// Update affected zero rows (revision mismatch or record not found).
    ZeroRows,
}

/// Check if an update affected any rows.
///
/// Returns [`UpdateResult::ZeroRows`] if no rows were updated, allowing callers to
/// disambiguate revision mismatch from missing record.
pub fn execute_optimistic_update(updated_rows: usize) -> UpdateResult {
    if updated_rows == 0 {
        UpdateResult::ZeroRows
    } else {
        UpdateResult::Success
    }
}

/// Disambiguate update failure by checking if it's a revision mismatch or missing record.
///
/// Given the result of querying for the current record, returns either a revision
/// mismatch error (if the record exists with different revision) or a not-found error
/// (if the record doesn't exist). Propagates any query errors.
pub fn disambiguate_update_failure<R>(
    current_result: Result<Option<R>, RouteAnnotationRepositoryError>,
    expected_revision: u32,
    not_found_message: &str,
) -> RouteAnnotationRepositoryError
where
    R: HasRevision,
{
    match current_result {
        Ok(Some(record)) => {
            RouteAnnotationRepositoryError::revision_mismatch(expected_revision, record.revision())
        }
        Ok(None) => RouteAnnotationRepositoryError::query(not_found_message),
        Err(e) => e,
    }
}

/// Collect row conversion results, mapping the first error through `map_err`.
pub fn collect_rows<T, E>(
    results: impl Iterator<Item = Result<T, String>>,
    map_err: impl FnOnce(String) -> E,
) -> Result<Vec<T>, E> {
    results.collect::<Result<Vec<_>, _>>().map_err(map_err)
}

/// Macro for query methods that return `Option<T>`.
///
/// Reduces boilerplate: acquire connection, execute query, map errors, convert row.
#[macro_export]
macro_rules! query_optional {
    (
        $self:ident,
        $table:expr,
        $filter:expr,
        $row_type:ty,
        $converter:expr
    ) => {{
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use $crate::outbound::persistence::diesel_helpers::map_diesel_error;
        use $crate::outbound::persistence::diesel_helpers::map_pool_error;

        let mut conn = $self.pool.get().await.map_err(map_pool_error)?;

        let result: Option<$row_type> = $table
            .filter($filter)
            .select(<$row_type>::as_select())
            .first(&mut conn)
            .await
            .optional()
            .map_err(map_diesel_error)?;

        Ok(result.map($converter))
    }};
}

/// Macro for query methods that return `Vec<T>`.
///
/// Reduces boilerplate: acquire connection, execute query with ordering, map errors, convert rows.
#[macro_export]
macro_rules! query_vec {
    (
        $self:ident,
        $table:expr,
        $filter:expr,
        $order_by:expr,
        $row_type:ty,
        $converter:expr
    ) => {{
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use $crate::outbound::persistence::diesel_helpers::map_diesel_error;
        use $crate::outbound::persistence::diesel_helpers::map_pool_error;

        let mut conn = $self.pool.get().await.map_err(map_pool_error)?;

        let rows: Vec<$row_type> = $table
            .filter($filter)
            .select(<$row_type>::as_select())
            .order_by($order_by)
            .load(&mut conn)
            .await
            .map_err(map_diesel_error)?;

        Ok(rows.into_iter().map($converter).collect())
    }};
}

/// Macro for querying from an existing connection and disambiguating update failures.
///
/// Reduces boilerplate in handle_*_update_failure functions: execute query with filter,
/// map errors, then disambiguate revision mismatch vs not-found.
#[macro_export]
macro_rules! query_and_disambiguate {
    (
        $conn:expr,
        $table:expr,
        $filter:expr,
        $row_type:ty,
        $expected_revision:expr,
        $not_found_msg:expr
    ) => {{
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use $crate::outbound::persistence::diesel_helpers::disambiguate_update_failure;
        use $crate::outbound::persistence::diesel_helpers::map_diesel_error;

        let current_result = $table
            .filter($filter)
            .select(<$row_type>::as_select())
            .first($conn)
            .await
            .optional()
            .map_err(map_diesel_error);

        disambiguate_update_failure(current_result, $expected_revision, $not_found_msg)
    }};
}

/// Macro for save operations with optimistic concurrency control.
///
/// Handles: acquire connection, insert (if None) or update with revision check
/// (if Some), disambiguate zero-row updates.
#[macro_export]
macro_rules! save_with_revision {
    (
        $self:ident,
        $expected_revision:expr,
        insert: { $($insert_body:tt)* },
        update($expected:ident): { $($update_body:tt)* }
    ) => {{
        use $crate::outbound::persistence::diesel_helpers::map_pool_error;

        let mut conn = $self.pool.get().await.map_err(map_pool_error)?;

        match $expected_revision {
            None => {
                save_with_revision!(@insert conn, { $($insert_body)* })
            }
            Some($expected) => {
                save_with_revision!(@update conn, $expected, { $($update_body)* })
            }
        }
    }};

    (@insert $conn:ident, {
        table: $table:expr,
        new_row: $new_row:expr
    }) => {{
        // Using #[allow] rather than #[expect] because the unused_imports lint does not
        // fire consistently for glob imports in macro expansion contexts. When the call
        // site already has `use diesel::prelude::*`, the lint should fire but doesn't,
        // causing #[expect] to fail with "unfulfilled lint expectation".
        #[allow(unused_imports, reason = "prelude may be imported at call site")]
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use $crate::outbound::persistence::diesel_helpers::map_diesel_error;

        diesel::insert_into($table)
            .values(&$new_row)
            .execute(&mut $conn)
            .await
            .map(|_| ())
            .map_err(map_diesel_error)
    }};

    (@update $conn:ident, $expected:ident, {
        table: $table:expr,
        filter: $filter:expr,
        changeset: $changeset:expr,
        on_zero_rows: $handler:expr
    }) => {{
        // See comment in @insert branch for why #[allow] is used instead of #[expect].
        #[allow(unused_imports, reason = "prelude may be imported at call site")]
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use $crate::outbound::persistence::diesel_helpers::map_diesel_error;
        use $crate::outbound::persistence::diesel_helpers::execute_optimistic_update;
        use $crate::outbound::persistence::diesel_helpers::UpdateResult;

        let changeset = $changeset;
        let updated_rows = diesel::update($table)
            .filter($filter)
            .set(&changeset)
            .execute(&mut $conn)
            .await
            .map_err(map_diesel_error)?;

        match execute_optimistic_update(updated_rows) {
            UpdateResult::ZeroRows => {
                return Err($handler(&mut $conn, $expected).await);
            }
            UpdateResult::Success => Ok(()),
        }
    }};
}
