//! Transport-agnostic domain errors.

use serde::{Deserialize, Serialize};
use serde_json::Value;
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

/// Domain-level error description independent of any transport.
///
/// ## Invariants
/// - `message` must be non-empty once trimmed of whitespace.
/// - `details`, when present, should contain client-safe context.
///
/// # Examples
/// ```
/// use backend::domain::{DomainError, ErrorCode};
///
/// let err = DomainError::new(ErrorCode::NotFound, "missing resource");
/// assert_eq!(err.code(), ErrorCode::NotFound);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
#[serde(try_from = "DomainErrorDto", into = "DomainErrorDto")]
#[error("{message}")]
pub struct DomainError {
    #[schema(example = "invalid_request")]
    code: ErrorCode,
    #[schema(example = "Something went wrong")]
    message: String,
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

impl DomainError {
    /// Create a new error.
    ///
    /// # Examples
    /// ```
    /// use backend::domain::{DomainError, ErrorCode};
    /// let err = DomainError::new(ErrorCode::InvalidRequest, "bad");
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

    /// Supplementary error details for clients.
    pub fn details(&self) -> Option<&Value> {
        self.details.as_ref()
    }

    /// Attach structured details to the error.
    ///
    /// # Examples
    /// ```
    /// use backend::domain::{DomainError, ErrorCode};
    /// use serde_json::json;
    /// let err = DomainError::new(ErrorCode::InvalidRequest, "bad")
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
    /// use backend::domain::DomainError;
    ///
    /// let err = DomainError::invalid_request("bad input");
    /// ```
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidRequest, message)
    }

    /// Convenience constructor for [`ErrorCode::Unauthorized`].
    ///
    /// # Examples
    /// ```
    /// use backend::domain::DomainError;
    ///
    /// let err = DomainError::unauthorized("no token");
    /// ```
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unauthorized, message)
    }

    /// Convenience constructor for [`ErrorCode::Forbidden`].
    ///
    /// # Examples
    /// ```
    /// use backend::domain::DomainError;
    ///
    /// let err = DomainError::forbidden("nope");
    /// ```
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Forbidden, message)
    }

    /// Convenience constructor for [`ErrorCode::NotFound`].
    ///
    /// # Examples
    /// ```
    /// use backend::domain::DomainError;
    ///
    /// let err = DomainError::not_found("missing");
    /// ```
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    /// Convenience constructor for [`ErrorCode::InternalError`].
    ///
    /// # Examples
    /// ```
    /// use backend::domain::DomainError;
    ///
    /// let err = DomainError::internal("boom");
    /// ```
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InternalError, message)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct DomainErrorDto {
    code: ErrorCode,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Value>,
}

impl From<DomainError> for DomainErrorDto {
    fn from(value: DomainError) -> Self {
        Self {
            code: value.code,
            message: value.message,
            details: value.details,
        }
    }
}

impl TryFrom<DomainErrorDto> for DomainError {
    type Error = ErrorValidationError;

    fn try_from(value: DomainErrorDto) -> Result<Self, Self::Error> {
        let DomainErrorDto {
            code,
            message,
            details,
        } = value;

        let mut error = DomainError::try_new(code, message)?;
        error.details = details;
        Ok(error)
    }
}

#[cfg(test)]
mod tests;
