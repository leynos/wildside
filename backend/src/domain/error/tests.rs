//! Tests for the error response payload formatting and propagation.

use super::*;
use crate::middleware::trace::TraceId;
use actix_web::{body::to_bytes, http::StatusCode};
use rstest::{fixture, rstest};
use rstest_bdd_macros::{given, then, when};
use serde_json::json;

const TRACE_ID: &str = "00000000-0000-0000-0000-000000000000";

#[fixture]
fn expected_trace_id() -> String {
    TRACE_ID.to_owned()
}

#[fixture]
fn base_error() -> Error {
    Error::invalid_request("bad")
}

#[fixture]
fn internal_error_case(expected_trace_id: String) -> Error {
    Error::internal("boom")
        .with_trace_id(expected_trace_id)
        .with_details(json!({"secret": "x"}))
}

#[fixture]
fn invalid_request_case(expected_trace_id: String) -> Error {
    Error::invalid_request("bad")
        .with_trace_id(expected_trace_id)
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
async fn new_captures_trace_id_in_scope(expected_trace_id: String) {
    let trace_id: TraceId = expected_trace_id
        .parse()
        .expect("fixtures provide a valid UUID");
    let error = TraceId::scope(trace_id, async move {
        Error::try_new(ErrorCode::InternalError, "boom")
            .expect("validation accepts non-empty message")
    })
    .await;

    assert_eq!(error.trace_id(), Some(expected_trace_id.as_str()));
}

#[rstest]
#[tokio::test]
async fn try_from_error_dto_clears_ambient_trace(expected_trace_id: String) {
    let trace_id: TraceId = expected_trace_id
        .parse()
        .expect("fixtures provide a valid UUID");
    let dto = ErrorDto {
        code: ErrorCode::InvalidRequest,
        message: "bad".to_string(),
        trace_id: None,
        details: None,
    };

    let error = TraceId::scope(trace_id, async move {
        Error::try_from(dto).expect("conversion succeeds for valid payload without trace")
    })
    .await;

    assert!(error.trace_id().is_none());
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
        .get(TRACE_ID_HEADER)
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
    expected_trace_id: String,
) {
    let redacted = assert_error_response(
        internal_error,
        StatusCode::INTERNAL_SERVER_ERROR,
        Some(expected_trace_id.as_str()),
    )
    .await;
    assert_eq!(redacted.code(), ErrorCode::InternalError);
    assert_eq!(redacted.message(), "Internal server error");
    assert!(redacted.details().is_none());

    let payload = assert_error_response(
        invalid_request,
        StatusCode::BAD_REQUEST,
        Some(expected_trace_id.as_str()),
    )
    .await;
    assert_eq!(payload.code(), ErrorCode::InvalidRequest);
    assert_eq!(payload.message(), "bad");
    assert_eq!(payload.details(), Some(&json!({"field": "name"})));
}

#[derive(Debug, Clone)]
enum ConstructedError {
    Success,
    Failure(ErrorValidationError),
}

impl ConstructedError {
    fn from_result(result: Result<Error, ErrorValidationError>) -> Self {
        match result {
            Ok(_) => Self::Success,
            Err(err) => Self::Failure(err),
        }
    }
}

#[given("a valid error payload")]
fn a_valid_error_payload() -> (ErrorCode, String) {
    (ErrorCode::InvalidRequest, "well formed".to_owned())
}

#[when("the error is constructed")]
fn the_error_is_constructed(payload: (ErrorCode, String)) -> ConstructedError {
    ConstructedError::from_result(Error::try_new(payload.0, payload.1))
}

#[then("the construction succeeds")]
fn the_construction_succeeds(result: ConstructedError) {
    assert!(matches!(result, ConstructedError::Success));
}

#[rstest]
fn constructing_an_error_happy_path() {
    let payload = a_valid_error_payload();
    let result = the_error_is_constructed((payload.0, payload.1.clone()));
    the_construction_succeeds(result);
}

#[given("an empty error message")]
fn an_empty_error_message() -> (ErrorCode, String) {
    (ErrorCode::InvalidRequest, "   ".to_owned())
}

#[then("construction fails with an empty message")]
fn construction_fails_with_empty_message(result: ConstructedError) {
    assert!(matches!(
        result,
        ConstructedError::Failure(ErrorValidationError::EmptyMessage)
    ));
}

#[rstest]
fn constructing_an_error_unhappy_path() {
    let payload = an_empty_error_message();
    let result = the_error_is_constructed((payload.0, payload.1.clone()));
    construction_fails_with_empty_message(result);
}
