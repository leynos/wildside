//! Error response types used by the HTTP API.
//!
//! Purpose: Provide a stable, documented structure for failures so clients can
//! react deterministically. The shape mirrors the OpenAPI specification and is
//! intentionally minimal.

use std::fmt;

use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

/// Payload describing an API failure.
///
/// The `code` allows clients to branch on machine readable categories while
/// `message` gives context for humans. Optional `trace_id` and `details` fields
/// assist debugging without leaking internal state.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(title = "Error", description = "An error response")]
pub struct Error {
    /// Stable machine readable error code.
    pub code: ErrorCode,
    /// Human-readable error message.
    pub message: String,
    /// Correlation identifier to trace this error across systems.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    /// Supplementary error details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

/// Known error categories.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The request is malformed or fails validation.
    InvalidRequest,
    /// Authentication failed or is missing.
    Unauthorized,
    /// Authenticated but not permitted to perform this action.
    Forbidden,
    /// The requested resource does not exist.
    NotFound,
    /// An unexpected error occurred on the server.
    InternalError,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = match self {
            Self::InvalidRequest => "invalid_request",
            Self::Unauthorized => "unauthorized",
            Self::Forbidden => "forbidden",
            Self::NotFound => "not_found",
            Self::InternalError => "internal_error",
        };
        f.write_str(code)
    }
}

impl ResponseError for Error {
    fn status_code(&self) -> StatusCode {
        match self.code {
            ErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrorCode::Forbidden => StatusCode::FORBIDDEN,
            ErrorCode::NotFound => StatusCode::NOT_FOUND,
            ErrorCode::InvalidRequest => StatusCode::BAD_REQUEST,
            ErrorCode::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).json(self)
    }
}
