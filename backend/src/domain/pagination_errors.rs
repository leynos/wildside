//! Shared pagination error envelope constructors.
//!
//! These helpers centralize the user-visible cursor error contract so inbound
//! adapters and repository error mapping cannot drift.

use serde_json::json;

use super::Error;

/// Build the standard invalid cursor error returned for malformed cursors.
pub(crate) fn invalid_cursor_error() -> Error {
    Error::invalid_request("cursor is invalid")
        .with_details(json!({ "field": "cursor", "code": "invalid_cursor" }))
}

/// Build the standard unsupported cursor direction error.
pub(crate) fn unsupported_direction_error() -> Error {
    Error::invalid_request("cursor direction is unsupported")
        .with_details(json!({ "field": "cursor", "code": "unsupported_direction" }))
}
