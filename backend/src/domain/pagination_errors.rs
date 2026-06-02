//! Shared pagination error envelope constructors.
//!
//! These helpers centralize the user-visible cursor error contract so inbound
//! adapters and repository error mapping cannot drift. The module returns
//! only domain [`Error`] values; logging and Prometheus side-effects live in
//! `crate::observability::pagination_errors` and are recorded by adapters.

use serde_json::json;

use super::Error;

/// Build the standard invalid-cursor error envelope.
pub(crate) fn invalid_cursor_error() -> Error {
    Error::invalid_request("cursor is invalid")
        .with_details(json!({ "field": "cursor", "code": "invalid_cursor" }))
}

/// Build the standard unsupported-direction error envelope.
pub(crate) fn unsupported_direction_error() -> Error {
    Error::invalid_request("cursor direction is unsupported")
        .with_details(json!({ "field": "cursor", "code": "unsupported_direction" }))
}
