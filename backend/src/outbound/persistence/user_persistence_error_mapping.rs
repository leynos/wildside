//! Shared mapping from user persistence failures to domain HTTP-safe errors.

use crate::domain::Error;
use crate::domain::pagination_errors::{invalid_cursor_error, unsupported_direction_error};
use crate::domain::ports::{UserPaginationError, UserPersistenceError};
use crate::observability::pagination_errors::{PaginationErrorSource, record_pagination_error};

pub(super) fn map_user_persistence_error(error: UserPersistenceError) -> Error {
    match error {
        UserPersistenceError::Connection { message } => Error::service_unavailable(message),
        UserPersistenceError::Query { message } => Error::internal(message),
        UserPersistenceError::Pagination { error } => map_user_pagination_error(error),
    }
}

fn map_user_pagination_error(error: UserPaginationError) -> Error {
    match error {
        UserPaginationError::InvalidCursorFormat { .. } => {
            record_pagination_error(PaginationErrorSource::UserPersistence, "invalid_cursor");
            invalid_cursor_error()
        }
        UserPaginationError::UnsupportedDirection { .. } => {
            record_pagination_error(
                PaginationErrorSource::UserPersistence,
                "unsupported_direction",
            );
            unsupported_direction_error()
        }
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for HTTP-safe user persistence error mapping.

    use super::*;
    use crate::domain::ErrorCode;
    use rstest::rstest;
    use serde_json::json;

    #[cfg(feature = "metrics")]
    use serial_test::serial;

    #[rstest]
    #[case(
        UserPersistenceError::pagination(UserPaginationError::invalid_cursor_format("bad token")),
        "cursor is invalid",
        "invalid_cursor"
    )]
    #[case(
        UserPersistenceError::pagination(UserPaginationError::unsupported_direction("sideways")),
        "cursor direction is unsupported",
        "unsupported_direction"
    )]
    #[cfg_attr(feature = "metrics", serial)]
    fn pagination_errors_map_to_invalid_request(
        #[case] source: UserPersistenceError,
        #[case] expected_message: &str,
        #[case] expected_detail_code: &str,
    ) {
        let error = map_user_persistence_error(source);

        assert_eq!(error.code(), ErrorCode::InvalidRequest);
        assert_eq!(error.message(), expected_message);
        assert_eq!(
            error.details(),
            Some(&json!({ "field": "cursor", "code": expected_detail_code }))
        );
    }
}
