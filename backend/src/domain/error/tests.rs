//! Tests for transport-agnostic domain errors.

use super::*;
use rstest::{fixture, rstest};
use rstest_bdd_macros::{given, then, when};
use serde_json::json;

#[fixture]
fn base_error() -> DomainError {
    DomainError::invalid_request("bad")
}

#[fixture]
fn detailed_error() -> DomainError {
    DomainError::invalid_request("bad").with_details(json!({"field": "name"}))
}

#[rstest]
fn invalid_request_constructor_sets_code(base_error: DomainError) {
    assert_eq!(base_error.code(), ErrorCode::InvalidRequest);
}

#[rstest]
fn try_new_rejects_empty_messages() {
    let result = DomainError::try_new(ErrorCode::InvalidRequest, "   ");
    assert!(matches!(result, Err(ErrorValidationError::EmptyMessage)));
}

#[rstest]
fn with_details_preserves_payload(detailed_error: DomainError) {
    assert_eq!(detailed_error.details(), Some(&json!({"field": "name"})));
}

#[rstest]
fn try_from_domain_error_dto_carries_details() {
    let dto = DomainErrorDto {
        code: ErrorCode::Forbidden,
        message: "forbidden".to_string(),
        details: Some(json!({"code": "nope"})),
    };

    let error = DomainError::try_from(dto).expect("valid payload converts");
    assert_eq!(error.code(), ErrorCode::Forbidden);
    assert_eq!(error.message(), "forbidden");
    assert_eq!(error.details(), Some(&json!({"code": "nope"})));
}

#[derive(Debug, Clone)]
enum ConstructedError {
    Success,
    Failure(ErrorValidationError),
}

impl ConstructedError {
    fn from_result(result: Result<DomainError, ErrorValidationError>) -> Self {
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
    ConstructedError::from_result(DomainError::try_new(payload.0, payload.1))
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
