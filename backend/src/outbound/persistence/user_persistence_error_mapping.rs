//! Shared mapping from user persistence failures to domain HTTP-safe errors.

use crate::domain::Error;
use crate::domain::ports::UserPersistenceError;

pub(super) fn map_user_persistence_error(error: UserPersistenceError) -> Error {
    match error {
        UserPersistenceError::Connection { message } => Error::service_unavailable(message),
        UserPersistenceError::Query { message } => Error::internal(message),
    }
}
