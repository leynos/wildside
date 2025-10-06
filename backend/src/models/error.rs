//! Error response types.

use crate::middleware::trace::TraceId;
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
///
/// # Examples
/// ```
/// use backend::models::{Error, ErrorCode};
///
/// let err = Error::new(ErrorCode::NotFound, "missing");
/// assert_eq!(err.code, ErrorCode::NotFound);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
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
    #[serde(alias = "trace_id")]
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
    /// Create a new error.
    ///
    /// Captures the current [`TraceId`](crate::middleware::trace::TraceId) if one
    /// is in scope so the error payload is correlated automatically.
    ///
    /// # Examples
    /// ```
    /// use backend::models::{Error, ErrorCode};
    /// let err = Error::new(ErrorCode::InvalidRequest, "bad");
    /// assert_eq!(err.code, ErrorCode::InvalidRequest);
    /// ```
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            trace_id: TraceId::current().map(|id| id.to_string()),
            details: None,
        }
    }

    /// Attach a trace identifier to the error.
    ///
    /// # Examples
    /// ```
    /// use backend::models::{Error, ErrorCode};
    /// let err = Error::new(ErrorCode::Forbidden, "nope").with_trace_id("abc");
    /// assert_eq!(err.trace_id.as_deref(), Some("abc"));
    /// ```
    pub fn with_trace_id(mut self, id: impl Into<String>) -> Self {
        self.trace_id = Some(id.into());
        self
    }

    /// Attach structured details to the error.
    ///
    /// # Examples
    /// ```
    /// use backend::models::{Error, ErrorCode};
    /// use serde_json::json;
    /// let err = Error::new(ErrorCode::InvalidRequest, "bad")
    ///     .with_details(json!({ "field": "name" }));
    /// assert!(err.details.is_some());
    /// ```
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Convenience constructor for [`ErrorCode::InvalidRequest`].
    ///
    /// # Examples
    /// ```
    /// use backend::models::Error;
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
    /// use backend::models::Error;
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
    /// use backend::models::Error;
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
    /// use backend::models::Error;
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
    /// use backend::models::Error;
    ///
    /// let err = Error::internal("boom");
    /// ```
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
            builder.insert_header(("trace-id", id.clone()));
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
#[cfg(test)]
mod tests {
    use super::*;
    use crate::middleware::trace::TraceId;
    use actix_web::{body::to_bytes, http::StatusCode};
    use serde_json::json;

    async fn assert_error_response(error: Error, expected_status: StatusCode) -> Error {
        let response = error.error_response();
        assert_eq!(response.status(), expected_status);

        let trace_id = response
            .headers()
            .get("trace-id")
            .or_else(|| response.headers().get("Trace-Id"))
            .expect("Trace-Id header is set by Error::error_response")
            .to_str()
            .expect("Trace-Id not valid UTF-8");
        assert_eq!(trace_id, "abc");

        let bytes = to_bytes(response.into_body())
            .await
            .expect("reading response body succeeds");

        serde_json::from_slice(&bytes).expect("Error JSON deserialisation succeeds")
    }

    #[test]
    fn invalid_request_constructor_sets_code() {
        let err = Error::invalid_request("bad");
        assert_eq!(err.code, ErrorCode::InvalidRequest);
    }

    #[tokio::test]
    async fn new_captures_trace_id_in_scope() {
        let trace_id: TraceId = "00000000-0000-0000-0000-000000000000".parse().unwrap();
        let expected = trace_id.to_string();
        let error = TraceId::scope(trace_id, async move {
            Error::new(ErrorCode::InternalError, "boom")
        })
        .await;
        assert_eq!(error.trace_id.as_deref(), Some(expected.as_str()));
    }

    #[test]
    fn new_returns_none_when_out_of_scope() {
        let error = Error::new(ErrorCode::InternalError, "boom");
        assert!(error.trace_id.is_none());
    }

    #[test]
    fn not_found_constructor_sets_code() {
        let err = Error::not_found("missing");
        assert_eq!(err.code, ErrorCode::NotFound);
    }
    #[test]
    fn status_code_matches_error_code() {
        use actix_web::http::StatusCode;
        let cases = [
            (Error::invalid_request("bad"), StatusCode::BAD_REQUEST),
            (Error::unauthorized("no auth"), StatusCode::UNAUTHORIZED),
            (Error::forbidden("denied"), StatusCode::FORBIDDEN),
            (Error::not_found("missing"), StatusCode::NOT_FOUND),
            (Error::internal("boom"), StatusCode::INTERNAL_SERVER_ERROR),
        ];
        for (err, status) in cases {
            assert_eq!(err.status_code(), status);
        }
    }

    #[actix_web::test]
    async fn internal_error_response_is_redacted() {
        let err = Error::internal("boom")
            .with_trace_id("abc")
            .with_details(json!({"secret": "x"}));
        let payload = assert_error_response(err, StatusCode::INTERNAL_SERVER_ERROR).await;
        assert_eq!(payload.message, "Internal server error");
        assert!(payload.details.is_none());
        assert_eq!(payload.trace_id.as_deref(), Some("abc"));
    }
    #[actix_web::test]
    async fn error_response_includes_details_and_header() {
        let err = Error::invalid_request("bad")
            .with_trace_id("abc")
            .with_details(json!({"field": "name"}));
        let payload = assert_error_response(err, StatusCode::BAD_REQUEST).await;
        assert_eq!(payload.code, ErrorCode::InvalidRequest);
        assert_eq!(payload.message, "bad");
        assert_eq!(payload.details, Some(json!({"field": "name"})));
        assert_eq!(payload.trace_id.as_deref(), Some("abc"));
    }
}
