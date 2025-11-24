//! Domain error validation and serde round-trips.

use super::*;
use pg_embedded_setup_unpriv::{test_support::test_cluster, TestCluster};
use postgres::NoTls;
use rstest::{fixture, rstest};
use rstest_bdd_macros::{given, then, when};
use serde_json::json;

#[fixture]
fn base_error() -> DomainError {
    DomainError::invalid_request("bad request body")
}

#[rstest]
fn invalid_request_constructor_sets_code() {
    let err = DomainError::invalid_request("bad");
    assert_eq!(err.code(), ErrorCode::InvalidRequest);
}

#[rstest]
fn try_new_rejects_empty_messages() {
    let result = DomainError::try_new(ErrorCode::InvalidRequest, "   ");
    assert!(matches!(
        result,
        Err(DomainErrorValidationError::EmptyMessage)
    ));
}

#[rstest]
fn with_details_attaches_payload(base_error: DomainError) {
    let err = base_error.with_details(json!({"field": "name"}));
    assert_eq!(err.details(), Some(&json!({"field": "name"})));
}

#[rstest]
fn serde_round_trip_preserves_fields() {
    let err = DomainError::forbidden("denied").with_details(json!({"reason": "policy"}));
    let json = serde_json::to_string(&err).expect("serialise");
    let round_tripped: DomainError =
        serde_json::from_str(&json).expect("deserialise should succeed");
    assert_eq!(round_tripped.code(), ErrorCode::Forbidden);
    assert_eq!(round_tripped.message(), "denied");
    assert_eq!(round_tripped.details(), Some(&json!({"reason": "policy"})));
}

#[rstest]
fn display_uses_message(base_error: DomainError) {
    assert_eq!(base_error.to_string(), base_error.message());
}

#[derive(Debug, Clone)]
enum ConstructedError {
    Success,
    Failure(DomainErrorValidationError),
}

impl ConstructedError {
    fn from_result(result: Result<DomainError, DomainErrorValidationError>) -> Self {
        match result {
            Ok(_) => Self::Success,
            Err(err) => Self::Failure(err),
        }
    }
}

#[given("a valid domain error payload")]
fn a_valid_domain_error_payload() -> (ErrorCode, String) {
    (ErrorCode::InvalidRequest, "well formed".to_owned())
}

#[when("the domain error is constructed")]
fn the_domain_error_is_constructed(payload: (ErrorCode, String)) -> ConstructedError {
    ConstructedError::from_result(DomainError::try_new(payload.0, payload.1))
}

#[then("the construction succeeds")]
fn the_construction_succeeds(result: ConstructedError) {
    assert!(matches!(result, ConstructedError::Success));
}

#[given("an empty error message")]
fn an_empty_error_message() -> (ErrorCode, String) {
    (ErrorCode::InvalidRequest, "   ".to_owned())
}

#[then("the construction fails with an empty message error")]
fn the_construction_fails(result: ConstructedError) {
    assert!(matches!(
        result,
        ConstructedError::Failure(DomainErrorValidationError::EmptyMessage)
    ));
}

#[rstest]
fn constructing_a_domain_error_happy_path() {
    let payload = a_valid_domain_error_payload();
    let result = the_domain_error_is_constructed(payload);
    the_construction_succeeds(result);
}

#[rstest]
fn constructing_a_domain_error_unhappy_path() {
    let payload = an_empty_error_message();
    let result = the_domain_error_is_constructed(payload);
    the_construction_fails(result);
}

#[rstest]
fn postgres_fixture_is_available(test_cluster: TestCluster) {
    let url = test_cluster.connection().database_url("postgres");
    let mut client = postgres::Client::connect(&url, NoTls).expect("connect to embedded postgres");
    let row = client
        .query_one("SELECT 1::INT", &[])
        .expect("query executes");
    let value: i32 = row.get(0);
    assert_eq!(value, 1);
}
