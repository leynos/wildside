//! Domain-level error types.
//!
//! These errors are transport agnostic. Inbound adapters map them to HTTP
//! responses, WebSocket frames, or any other protocol-specific envelope.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

/// Stable machine-readable error code describing the failure category.
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
    /// An unexpected error occurred inside the domain.
    InternalError,
}

/// Domain error payload.
///
/// ## Invariants
/// - `message` must be non-empty once trimmed of whitespace.
///
/// # Examples
/// ```
/// use backend::domain::{DomainError, ErrorCode};
///
/// let err = DomainError::new(ErrorCode::NotFound, "missing");
/// assert_eq!(err.code(), ErrorCode::NotFound);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
#[serde(try_from = "DomainErrorDto", into = "DomainErrorDto")]
pub struct DomainError {
    #[schema(example = "invalid_request")]
    code: ErrorCode,
    #[schema(example = "Something went wrong")]
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Value>,
}

/// Validation errors emitted by the constructors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainErrorValidationError {
    EmptyMessage,
}

impl std::fmt::Display for DomainErrorValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyMessage => write!(f, "error message must not be empty"),
        }
    }
}

impl std::error::Error for DomainErrorValidationError {}

impl DomainError {
    /// Create a new error, panicking if validation fails.
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
    ) -> Result<Self, DomainErrorValidationError> {
        let message = message.into();
        if message.trim().is_empty() {
            return Err(DomainErrorValidationError::EmptyMessage);
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

    /// Human-readable message returned to adapters.
    pub fn message(&self) -> &str {
        self.message.as_str()
    }

    /// Supplementary error details for adapters.
    pub fn details(&self) -> Option<&Value> {
        self.details.as_ref()
    }

    /// Attach structured details to the error.
    ///
    /// # Examples
    /// ```
    /// use backend::domain::{DomainError, ErrorCode};
    /// use serde_json::json;
    ///
    /// let err = DomainError::new(ErrorCode::InvalidRequest, "bad")
    ///     .with_details(json!({ "field": "name" }));
    /// assert!(err.details().is_some());
    /// ```
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Convenience constructor for [`ErrorCode::InvalidRequest`].
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidRequest, message)
    }

    /// Convenience constructor for [`ErrorCode::Unauthorized`].
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unauthorized, message)
    }

    /// Convenience constructor for [`ErrorCode::Forbidden`].
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Forbidden, message)
    }

    /// Convenience constructor for [`ErrorCode::NotFound`].
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    /// Convenience constructor for [`ErrorCode::InternalError`].
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InternalError, message)
    }
}

impl std::fmt::Display for DomainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for DomainError {}

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
    type Error = DomainErrorValidationError;

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
