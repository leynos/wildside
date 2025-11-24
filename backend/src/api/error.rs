//! HTTP adapter error mapping.
//!
//! Purpose: convert transport-agnostic domain errors into HTTP responses while
//! attaching the current trace identifier for diagnostics. The domain stays
//! unaware of Actix or HTTP semantics; this module performs the translation
//! and owns the response payload shape exposed to clients.

use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::error;
use utoipa::ToSchema;

use crate::domain::{DomainError, ErrorCode};
use crate::middleware::trace::{TraceId, TRACE_ID_HEADER};

/// API error response payload.
///
/// ## Invariants
/// - `message` must be non-empty once trimmed of whitespace.
/// - `trace_id`, when present, must be non-empty.
///
/// # Examples
/// ```
/// use backend::api::error::ApiError;
/// use backend::domain::{DomainError, ErrorCode};
///
/// let domain_err = DomainError::not_found("missing");
/// let api_err = ApiError::from(domain_err);
/// assert_eq!(api_err.code(), ErrorCode::NotFound);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
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

impl ApiError {
    /// Stable machine-readable error code.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        self.code
    }

    /// Human-readable message returned to clients.
    #[must_use]
    pub fn message(&self) -> &str {
        self.message.as_str()
    }

    /// Correlation identifier for tracing this error across systems.
    #[must_use]
    pub fn trace_id(&self) -> Option<&str> {
        self.trace_id.as_deref()
    }

    /// Supplementary error details for clients.
    #[must_use]
    pub fn details(&self) -> Option<&Value> {
        self.details.as_ref()
    }

    fn http_status(&self) -> StatusCode {
        status_code_for(self.code)
    }
}

impl From<DomainError> for ApiError {
    fn from(error: DomainError) -> Self {
        Self {
            code: error.code(),
            message: error.message().to_owned(),
            details: error.details().cloned(),
            trace_id: TraceId::current().map(|id| id.to_string()),
        }
    }
}

impl From<actix_web::Error> for ApiError {
    fn from(err: actix_web::Error) -> Self {
        error!(error = %err, "actix error promoted to API error");
        DomainError::internal("Internal server error").into()
    }
}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        self.http_status()
    }

    fn error_response(&self) -> HttpResponse {
        let mut builder = HttpResponse::build(self.http_status());
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
    type Error = crate::domain::ErrorValidationError;

    fn try_from(value: ApiErrorDto) -> Result<Self, Self::Error> {
        let ApiErrorDto {
            code,
            message,
            trace_id,
            details,
        } = value;

        let domain = DomainError::try_new(code, message)?;
        let mut api_error: ApiError = domain.into();
        if let Some(id) = trace_id {
            if id.trim().is_empty() {
                return Err(crate::domain::ErrorValidationError::EmptyTraceId);
            }
            api_error.trace_id = Some(id);
        }
        api_error.details = details;
        Ok(api_error)
    }
}

fn status_code_for(code: ErrorCode) -> StatusCode {
    match code {
        ErrorCode::InvalidRequest => StatusCode::BAD_REQUEST,
        ErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
        ErrorCode::Forbidden => StatusCode::FORBIDDEN,
        ErrorCode::NotFound => StatusCode::NOT_FOUND,
        ErrorCode::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Convenient HTTP result alias.
pub type ApiResult<T> = Result<T, ApiError>;

#[cfg(test)]
mod tests;
