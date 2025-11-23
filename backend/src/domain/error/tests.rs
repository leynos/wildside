//! Tests for the error payload validation helpers.

use super::*;
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
fn with_optional_trace_id_sets_value(base_error: Error, expected_trace_id: String) {
    let error = base_error
        .with_optional_trace_id(Some(expected_trace_id.clone()))
        .expect("trace id should attach");
    assert_eq!(error.trace_id(), Some(expected_trace_id.as_str()));
}

#[rstest]
fn with_optional_trace_id_is_noop_when_absent(base_error: Error) {
    let error = base_error
        .with_optional_trace_id(None)
        .expect("missing trace id should be allowed");
    assert!(error.trace_id().is_none());
}

#[rstest]
fn redacted_internal_errors_drop_details(
    #[from(internal_error_case)] internal_error: Error,
    expected_trace_id: String,
) {
    let redacted = internal_error.redacted_for_clients();
    assert_eq!(redacted.message(), "Internal server error");
    assert_eq!(redacted.trace_id(), Some(expected_trace_id.as_str()));
    assert!(redacted.details().is_none());
}

#[rstest]
fn redacted_non_internal_errors_are_unchanged(
    #[from(invalid_request_case)] invalid_request: Error,
) {
    let redacted = invalid_request.redacted_for_clients();
    assert_eq!(redacted.message(), invalid_request.message());
    assert_eq!(redacted.details(), invalid_request.details());
    assert_eq!(redacted.trace_id(), invalid_request.trace_id());
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
