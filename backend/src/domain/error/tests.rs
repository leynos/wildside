//! Tests for constructing and validating domain error payloads.

use super::*;
use crate::middleware::trace::TraceId;
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
async fn try_from_error_dto_preserves_payload(expected_trace_id: String) {
    let trace_id: TraceId = expected_trace_id
        .parse()
        .expect("fixtures provide a valid UUID");
    let dto = ErrorDto {
        code: ErrorCode::InvalidRequest,
        message: "bad".to_string(),
        trace_id: Some(trace_id.to_string()),
        details: Some(json!({"field": "name"})),
    };

    let error = TraceId::scope(trace_id, async move {
        Error::try_from(dto).expect("conversion succeeds for valid payload")
    })
    .await;

    assert_eq!(error.code(), ErrorCode::InvalidRequest);
    assert_eq!(error.message(), "bad");
    assert_eq!(error.trace_id(), Some(expected_trace_id.as_str()));
    assert_eq!(error.details(), Some(&json!({"field": "name"})));
}

#[rstest]
fn with_details_attaches_payload() {
    let details = json!({"field": "name"});
    let err = Error::invalid_request("bad").with_details(details.clone());
    assert_eq!(err.details(), Some(&details));
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
