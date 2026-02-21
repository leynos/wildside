//! Shared Diesel error mapping for repositories with basic query semantics.

use tracing::debug;

use super::pool::PoolError;

/// Map pool errors into a repository-specific connection error constructor.
pub fn map_basic_pool_error<E, C>(error: PoolError, connection: C) -> E
where
    C: FnOnce(String) -> E,
{
    let message = match error {
        PoolError::Checkout { message } | PoolError::Build { message } => message,
    };
    connection(message)
}

/// Map common Diesel error variants into query/connection constructors.
///
/// This helper captures the repeated mapping used by repositories where
/// `NotFound` and query-builder failures should map to query errors.
pub fn map_basic_diesel_error<E, Q, C>(error: diesel::result::Error, query: Q, connection: C) -> E
where
    Q: Fn(&'static str) -> E,
    C: Fn(&'static str) -> E,
{
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
        DieselError::NotFound => query("record not found"),
        DieselError::QueryBuilderError(_) => query("database query error"),
        DieselError::DatabaseError(DatabaseErrorKind::ClosedConnection, _) => {
            connection("database connection error")
        }
        DieselError::DatabaseError(_, _) => query("database error"),
        _ => query("database error"),
    }
}
