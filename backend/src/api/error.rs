//! HTTP error payloads and mapping from domain errors.
//!
//! Keep the domain free of transport concerns by translating [`DomainError`]
//! into Actix responses here.

use crate::domain::{DomainError, ErrorCode, TRACE_ID_HEADER};
use crate::middleware::trace::TraceId;
use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::error;
use utoipa::ToSchema;

/// Standard error envelope returned by HTTP adapters.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
#[serde(try_from = "ApiErrorDto", into = "ApiErrorDto")]
pub struct ApiError {
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

/// Validation failures raised when constructing an [`ApiError`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiErrorValidationError {
    EmptyMessage,
    EmptyTraceId,
}

impl std::fmt::Display for ApiErrorValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyMessage => write!(f, "error message must not be empty"),
            Self::EmptyTraceId => write!(f, "trace identifier must not be empty"),
        }
    }
}

impl std::error::Error for ApiErrorValidationError {}

impl ApiError {
    /// Construct an API error from a domain failure, capturing any ambient trace
    /// identifier.
    pub fn from_domain(error: DomainError) -> Self {
        Self {
            code: error.code(),
            message: error.message().to_owned(),
            trace_id: TraceId::current().map(|id| id.to_string()),
            details: error.details().cloned(),
        }
    }

    /// Fallible constructor used by serde conversions.
    pub fn try_new(
        code: ErrorCode,
        message: impl Into<String>,
    ) -> Result<Self, ApiErrorValidationError> {
        let message = message.into();
        if message.trim().is_empty() {
            return Err(ApiErrorValidationError::EmptyMessage);
        }
        Ok(Self {
            code,
            message,
            trace_id: TraceId::current().map(|id| id.to_string()),
            details: None,
        })
    }

    /// Attach structured details to the error.
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Stable machine-readable error code.
    pub fn code(&self) -> ErrorCode {
        self.code
    }

    /// Human readable message.
    pub fn message(&self) -> &str {
        self.message.as_str()
    }

    /// Trace identifier propagated into the response header.
    pub fn trace_id(&self) -> Option<&str> {
        self.trace_id.as_deref()
    }

    /// Supplementary error details for clients.
    pub fn details(&self) -> Option<&Value> {
        self.details.as_ref()
    }

    fn to_status_code(&self) -> StatusCode {
        match self.code {
            ErrorCode::InvalidRequest => StatusCode::BAD_REQUEST,
            ErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrorCode::Forbidden => StatusCode::FORBIDDEN,
            ErrorCode::NotFound => StatusCode::NOT_FOUND,
            ErrorCode::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<DomainError> for ApiError {
    fn from(value: DomainError) -> Self {
        ApiError::from_domain(value)
    }
}

impl From<actix_web::Error> for ApiError {
    fn from(err: actix_web::Error) -> Self {
        error!(error = %err, "actix error promoted to API error");
        ApiError {
            code: ErrorCode::InternalError,
            message: "Internal server error".to_string(),
            trace_id: TraceId::current().map(|id| id.to_string()),
            details: None,
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ApiError {}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        self.to_status_code()
    }

    fn error_response(&self) -> HttpResponse {
        let mut builder = HttpResponse::build(self.status_code());
        if let Some(id) = &self.trace_id {
            builder.insert_header((TRACE_ID_HEADER, id.clone()));
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

/// Convenience alias for HTTP handlers.
pub type ApiResult<T> = Result<T, ApiError>;

/// Adapter-level mapping from domain failures to HTTP payloads.
pub fn map_domain_error(error: DomainError) -> ApiError {
    ApiError::from(error)
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ApiErrorDto {
    code: ErrorCode,
    message: String,
    #[serde(alias = "trace_id")]
    #[schema(example = "01HZY8B2W6X5Y7Z9ABCD1234")]
    #[serde(skip_serializing_if = "Option::is_none")]
    trace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Value>,
}

impl From<ApiError> for ApiErrorDto {
    fn from(value: ApiError) -> Self {
        Self {
            code: value.code,
            message: value.message,
            trace_id: value.trace_id,
            details: value.details,
        }
    }
}

impl TryFrom<ApiErrorDto> for ApiError {
    type Error = ApiErrorValidationError;

    fn try_from(value: ApiErrorDto) -> Result<Self, Self::Error> {
        let ApiErrorDto {
            code,
            message,
            trace_id,
            details,
        } = value;

        let mut error = ApiError::try_new(code, message)?;
        if let Some(trace_id) = trace_id {
            if trace_id.trim().is_empty() {
                return Err(ApiErrorValidationError::EmptyTraceId);
            }
            error.trace_id = Some(trace_id);
        } else {
            error.trace_id = None;
        }
        error.details = details;
        Ok(error)
    }
}

#[cfg(test)]
mod tests;
