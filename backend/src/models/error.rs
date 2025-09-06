//! Error response types.

use crate::middleware::trace;
use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::error;
use utoipa::ToSchema;

/// Stable machine-readable error code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
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

/// API error response payload.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Error {
    /// Stable machine-readable error code.
    #[schema(example = "invalid_request")]
    pub code: ErrorCode,
    /// Human-readable error message.
    #[schema(example = "Something went wrong")]
    pub message: String,
    /// Correlation identifier for tracing this error across systems.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "01HZY8B2W6X5Y7Z9ABCD1234")]
    pub trace_id: Option<String>,
    /// Supplementary error details.
    ///
    /// This field should contain additional structured information about the error,
    /// such as validation errors, field-specific issues, or other context.
    /// The expected format is a JSON object, for example:
    /// `{ "field_errors": { "email": "invalid format" }, "reason": "missing data" }`
    /// Consumers should document and maintain the structure of this object for consistency.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl Error {
    /// Create a new error with the current trace identifier.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            trace_id: trace::current_trace_id(),
            details: None,
        }
    }

    /// Attach a trace identifier to the error.
    pub fn with_trace_id(mut self, id: impl Into<String>) -> Self {
        self.trace_id = Some(id.into());
        self
    }

    /// Attach structured details to the error.
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unauthorized, message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Forbidden, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InternalError, message)
    }
}

impl From<actix_web::Error> for Error {
    fn from(err: actix_web::Error) -> Self {
        // Do not leak implementation details to clients.
        error!(error = %err, "actix error promoted to API error");
        Error::internal("Internal server error")
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for Error {}

impl ErrorCode {
    fn as_status_code(&self) -> StatusCode {
        match self {
            ErrorCode::InvalidRequest => StatusCode::BAD_REQUEST,
            ErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrorCode::Forbidden => StatusCode::FORBIDDEN,
            ErrorCode::NotFound => StatusCode::NOT_FOUND,
            ErrorCode::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl ResponseError for Error {
    fn status_code(&self) -> StatusCode {
        self.code.as_status_code()
    }

    fn error_response(&self) -> HttpResponse {
        let mut builder = HttpResponse::build(self.status_code());
        if let Some(id) = &self.trace_id {
            builder.insert_header(("Trace-Id", id.clone()));
        }
        if matches!(self.code, ErrorCode::InternalError) {
            let mut redacted = self.clone();
            redacted.message = "Internal server error".to_string();
            redacted.details = None;
            return builder.json(redacted);
        }
        builder.json(self)
    }
}
