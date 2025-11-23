//! Domain error representation shared across adapters.
//!
//! Keep this module free from HTTP or framework concerns so the same error
//! shape can be mapped by any adapter (HTTP, WebSocket, background workers).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
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

/// Domain error payload consumed by adapters.
///
/// ## Invariants
/// - `message` must be non-empty once trimmed of whitespace.
/// - `trace_id`, when present, must be non-empty.
///
/// # Examples
/// ```
/// use backend::domain::{Error, ErrorCode};
///
/// let err = Error::new(ErrorCode::NotFound, "missing");
/// assert_eq!(err.code(), ErrorCode::NotFound);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema, Error)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
#[serde(try_from = "ErrorDto", into = "ErrorDto")]
#[error("{message}")]
pub struct Error {
    #[schema(example = "invalid_request")]
    code: ErrorCode,
    #[schema(example = "Something went wrong")]
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "01HZY8B2W6X5Y7Z9ABCD1234")]
    #[serde(alias = "trace_id")]
    trace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorValidationError {
    EmptyMessage,
    EmptyTraceId,
}

impl std::fmt::Display for ErrorValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyMessage => write!(f, "error message must not be empty"),
            Self::EmptyTraceId => write!(f, "trace identifier must not be empty"),
        }
    }
}

impl std::error::Error for ErrorValidationError {}

impl Error {
    /// Create a new error.
    ///
    /// # Examples
    /// ```
    /// use backend::domain::{Error, ErrorCode};
    /// let err = Error::new(ErrorCode::InvalidRequest, "bad");
    /// assert_eq!(err.code(), ErrorCode::InvalidRequest);
    /// ```
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        match Self::try_new(code, message) {
            Ok(value) => value,
            Err(err) => panic!("error messages must satisfy validation: {err}"),
        }
    }

    /// Fallible constructor that validates the message content.
    pub fn try_new(
        code: ErrorCode,
        message: impl Into<String>,
    ) -> Result<Self, ErrorValidationError> {
        let message = message.into();
        if message.trim().is_empty() {
            return Err(ErrorValidationError::EmptyMessage);
        }
        Ok(Self {
            code,
            message,
            trace_id: None,
            details: None,
        })
    }

    /// Stable machine-readable error code.
    pub fn code(&self) -> ErrorCode {
        self.code
    }

    /// Human-readable message returned to clients.
    pub fn message(&self) -> &str {
        self.message.as_str()
    }

    /// Correlation identifier for tracing this error across systems.
    pub fn trace_id(&self) -> Option<&str> {
        self.trace_id.as_deref()
    }

    /// Supplementary error details for clients.
    pub fn details(&self) -> Option<&Value> {
        self.details.as_ref()
    }

    /// Attach a trace identifier to the error.
    ///
    /// # Examples
    /// ```
    /// use backend::domain::{Error, ErrorCode};
    /// let err = Error::new(ErrorCode::Forbidden, "nope").with_trace_id("abc");
    /// assert_eq!(err.trace_id(), Some("abc"));
    /// ```
    pub fn with_trace_id(self, id: impl Into<String>) -> Self {
        match self.try_with_trace_id(id) {
            Ok(value) => value,
            Err(err) => panic!("trace identifiers must satisfy validation: {err}"),
        }
    }

    /// Fallible variant of [`Self::with_trace_id`].
    pub fn try_with_trace_id(
        mut self,
        id: impl Into<String>,
    ) -> Result<Self, ErrorValidationError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(ErrorValidationError::EmptyTraceId);
        }
        self.trace_id = Some(id);
        Ok(self)
    }

    /// Attach structured details to the error.
    ///
    /// # Examples
    /// ```
    /// use backend::domain::{Error, ErrorCode};
    /// use serde_json::json;
    /// let err = Error::new(ErrorCode::InvalidRequest, "bad")
    ///     .with_details(json!({ "field": "name" }));
    /// assert!(err.details().is_some());
    /// ```
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Convenience constructor for [`ErrorCode::InvalidRequest`].
    ///
    /// # Examples
    /// ```
    /// use backend::domain::Error;
    ///
    /// let err = Error::invalid_request("bad input");
    /// ```
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidRequest, message)
    }

    /// Convenience constructor for [`ErrorCode::Unauthorized`].
    ///
    /// # Examples
    /// ```
    /// use backend::domain::Error;
    ///
    /// let err = Error::unauthorized("no token");
    /// ```
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unauthorized, message)
    }

    /// Convenience constructor for [`ErrorCode::Forbidden`].
    ///
    /// # Examples
    /// ```
    /// use backend::domain::Error;
    ///
    /// let err = Error::forbidden("nope");
    /// ```
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Forbidden, message)
    }

    /// Convenience constructor for [`ErrorCode::NotFound`].
    ///
    /// # Examples
    /// ```
    /// use backend::domain::Error;
    ///
    /// let err = Error::not_found("missing");
    /// ```
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    /// Convenience constructor for [`ErrorCode::InternalError`].
    ///
    /// # Examples
    /// ```
    /// use backend::domain::Error;
    ///
    /// let err = Error::internal("boom");
    /// ```
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InternalError, message)
    }

    /// Provide a trace identifier if absent.
    pub fn with_optional_trace_id(
        self,
        trace_id: Option<String>,
    ) -> Result<Self, ErrorValidationError> {
        match trace_id {
            Some(id) => self.try_with_trace_id(id),
            None => Ok(self),
        }
    }

    /// Redact server-side details so the payload is safe to expose to clients.
    ///
    /// Internal errors keep their code and trace identifier but replace the
    /// message with a generic explanation and drop structured details.
    pub fn redacted_for_clients(&self) -> Self {
        if !matches!(self.code, ErrorCode::InternalError) {
            return self.clone();
        }
        let mut redacted = self.clone();
        redacted.message = "Internal server error".to_string();
        redacted.details = None;
        redacted
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ErrorDto {
    code: ErrorCode,
    message: String,
    #[serde(alias = "trace_id")]
    #[schema(example = "01HZY8B2W6X5Y7Z9ABCD1234")]
    #[serde(skip_serializing_if = "Option::is_none")]
    trace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Value>,
}

impl From<Error> for ErrorDto {
    fn from(value: Error) -> Self {
        Self {
            code: value.code,
            message: value.message,
            trace_id: value.trace_id,
            details: value.details,
        }
    }
}

impl TryFrom<ErrorDto> for Error {
    type Error = ErrorValidationError;

    fn try_from(value: ErrorDto) -> Result<Self, Self::Error> {
        let ErrorDto {
            code,
            message,
            trace_id,
            details,
        } = value;

        let mut error = Error::try_new(code, message)?;
        if let Some(trace_id) = trace_id {
            error = error.try_with_trace_id(trace_id)?;
        } else {
            error.trace_id = None;
        }
        error.details = details;
        Ok(error)
    }
}

#[cfg(test)]
mod tests;
