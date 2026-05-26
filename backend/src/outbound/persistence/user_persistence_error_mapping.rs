//! Shared mapping from user persistence failures to domain HTTP-safe errors.

use crate::domain::Error;
use crate::domain::ports::{UserPaginationError, UserPersistenceError};
use serde_json::json;

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
            Error::invalid_request("cursor is invalid")
                .with_details(json!({ "field": "cursor", "code": "invalid_cursor" }))
        }
        UserPaginationError::UnsupportedDirection { .. } => {
            Error::invalid_request("cursor direction is unsupported")
                .with_details(json!({ "field": "cursor", "code": "unsupported_direction" }))
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
