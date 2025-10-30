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
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
#[serde(try_from = "ErrorDto", into = "ErrorDto")]
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
    /// Captures the current trace identifier if one is in scope so the error
    /// payload is correlated automatically.
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
            trace_id: TraceId::current().map(|id| id.to_string()),
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
        let mut error = Error::try_new(value.code, value.message)?;
        if let Some(trace_id) = value.trace_id {
            error = error.try_with_trace_id(trace_id)?;
        }
        error.details = value.details;
        Ok(error)
    }
}

#[cfg(test)]
mod tests {
    //! Tests for the error response payload formatting and propagation.

    use super::*;
    use crate::middleware::trace::TraceId;
    use actix_web::{body::to_bytes, http::StatusCode};
    use rstest::{fixture, rstest};
    use rstest_bdd::{given, then, when};
    use serde_json::json;

    const TRACE_ID: &str = "00000000-0000-0000-0000-000000000000";

    #[fixture]
    fn expected_trace_id() -> &'static str {
        TRACE_ID
    }

    #[fixture]
    fn base_error() -> Error {
        Error::invalid_request("bad")
    }

    #[fixture]
    fn internal_error_case(expected_trace_id: &'static str) -> Error {
        Error::internal("boom")
            .with_trace_id(expected_trace_id.to_owned())
            .with_details(json!({"secret": "x"}))
    }

    #[fixture]
    fn invalid_request_case(expected_trace_id: &'static str) -> Error {
        Error::invalid_request("bad")
            .with_trace_id(expected_trace_id.to_owned())
            .with_details(json!({"field": "name"}))
    }

    #[rstest]
    fn invalid_request_constructor_sets_code() {
        let err = Error::invalid_request("bad");
        assert_eq!(err.code(), ErrorCode::InvalidRequest);
    }

    #[rstest]
    fn try_new_rejects_empty_messages() {
        let result = Error::try_new(ErrorCode::InvalidRequest, "   ");
        assert!(matches!(result, Err(ErrorValidationError::EmptyMessage)));
    }

    #[rstest]
    fn try_with_trace_id_rejects_empty_values(base_error: Error) {
        let result = base_error.try_with_trace_id("   ");
        assert!(matches!(result, Err(ErrorValidationError::EmptyTraceId)));
    }

    #[rstest]
    fn new_returns_none_when_trace_id_out_of_scope() {
        let error = Error::internal("boom");
        assert!(error.trace_id().is_none());
    }

    #[rstest]
    #[tokio::test]
    async fn new_captures_trace_id_in_scope(expected_trace_id: &'static str) {
        let trace_id: TraceId = expected_trace_id
            .parse()
            .expect("fixtures provide a valid UUID");
        let error = TraceId::scope(trace_id, async move {
            Error::try_new(ErrorCode::InternalError, "boom")
                .expect("validation accepts non-empty message")
        })
        .await;

        assert_eq!(error.trace_id(), Some(expected_trace_id));
    }

    #[rstest]
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

    async fn assert_error_response(
        error: Error,
        expected_status: StatusCode,
        expected_trace_id: Option<&str>,
    ) -> Error {
        let response = error.error_response();
        assert_eq!(response.status(), expected_status);

        let header = response
            .headers()
            .get("trace-id")
            .or_else(|| response.headers().get("Trace-Id"));
        match expected_trace_id {
            Some(expected) => {
                let trace_id = header
                    .expect("Trace-Id header is set by Error::error_response")
                    .to_str()
                    .expect("Trace-Id not valid UTF-8");
                assert_eq!(trace_id, expected);
            }
            None => {
                assert!(header.is_none(), "Trace-Id header should not be present");
            }
        }

        let bytes = to_bytes(response.into_body())
            .await
            .expect("reading response body succeeds");

        serde_json::from_slice(&bytes).expect("Error JSON deserialisation succeeds")
    }

    #[rstest]
    #[actix_web::test]
    async fn error_responses_include_trace_id_and_payloads(
        #[from(internal_error_case)] internal_error: Error,
        #[from(invalid_request_case)] invalid_request: Error,
        expected_trace_id: &'static str,
    ) {
        let redacted = assert_error_response(
            internal_error,
            StatusCode::INTERNAL_SERVER_ERROR,
            Some(expected_trace_id),
        )
        .await;
        assert_eq!(redacted.code(), ErrorCode::InternalError);
        assert_eq!(redacted.message(), "Internal server error");
        assert!(redacted.details().is_none());

        let payload = assert_error_response(
            invalid_request,
            StatusCode::BAD_REQUEST,
            Some(expected_trace_id),
        )
        .await;
        assert_eq!(payload.code(), ErrorCode::InvalidRequest);
        assert_eq!(payload.message(), "bad");
        assert_eq!(payload.details(), Some(&json!({"field": "name"})));
    }

    #[given("a valid error payload")]
    fn a_valid_error_payload() -> (ErrorCode, &'static str) {
        (ErrorCode::InvalidRequest, "well formed")
    }

    #[when("the error is constructed")]
    fn the_error_is_constructed(
        payload: (ErrorCode, &'static str),
    ) -> Result<Error, ErrorValidationError> {
        Error::try_new(payload.0, payload.1)
    }

    #[then("the construction succeeds")]
    fn the_construction_succeeds(result: Result<Error, ErrorValidationError>) {
        assert!(result.is_ok());
    }

    #[rstest]
    fn constructing_an_error_happy_path() {
        let payload = a_valid_error_payload();
        let result = the_error_is_constructed(payload);
        the_construction_succeeds(result);
    }

    #[given("an empty error message")]
    fn an_empty_error_message() -> (ErrorCode, &'static str) {
        (ErrorCode::InvalidRequest, "   ")
    }

    #[then("construction fails with an empty message")]
    fn construction_fails_with_empty_message(result: Result<Error, ErrorValidationError>) {
        assert!(matches!(result, Err(ErrorValidationError::EmptyMessage)));
    }

    #[rstest]
    fn constructing_an_error_unhappy_path() {
        let payload = an_empty_error_message();
        let result = the_error_is_constructed(payload);
        construction_fails_with_empty_message(result);
    }
}
