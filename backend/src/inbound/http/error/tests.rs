//! Tests for HTTP error mapping.

use super::*;
use crate::domain::Error;
use actix_web::body::to_bytes;
use actix_web::http::StatusCode;
use actix_web::ResponseError;
use rstest::{fixture, rstest};
use rstest_bdd_macros::{given, then, when};
use serde_json::json;

const TRACE_ID: &str = "00000000-0000-0000-0000-000000000000";

#[fixture]
fn expected_trace_id() -> String {
    TRACE_ID.to_owned()
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
fn status_code_matches_error_code() {
    let cases = [
        (Error::invalid_request("bad"), StatusCode::BAD_REQUEST),
        (Error::unauthorized("no auth"), StatusCode::UNAUTHORIZED),
        (Error::forbidden("denied"), StatusCode::FORBIDDEN),
        (Error::not_found("missing"), StatusCode::NOT_FOUND),
        (Error::internal("boom"), StatusCode::INTERNAL_SERVER_ERROR),
    ];
    for (err, status) in cases {
        assert_eq!(ResponseError::status_code(&err), status);
    }
}

async fn assert_error_response(
    error: Error,
    expected_status: StatusCode,
    expected_trace_id: Option<&str>,
) -> Error {
    let response = ResponseError::error_response(&error);
    assert_eq!(response.status(), expected_status);

    let header = response
        .headers()
        .get(TRACE_ID_HEADER)
        .or_else(|| response.headers().get("Trace-Id"));
    match expected_trace_id {
        Some(expected) => {
            let trace_id = header
                .expect("Trace-Id header is set by error_response")
                .to_str()
                .expect("Trace-Id not valid UTF-8");
            assert_eq!(trace_id, expected);
        }
        None => assert!(header.is_none(), "Trace-Id header should not be present"),
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

#[rstest]
#[actix_web::test]
async fn error_without_trace_id_omits_trace_header() {
    let error = Error::invalid_request("bad").with_details(json!({"field": "name"}));

    let payload = assert_error_response(error, StatusCode::BAD_REQUEST, None).await;
    assert_eq!(payload.code(), ErrorCode::InvalidRequest);
    assert_eq!(payload.message(), "bad");
    assert_eq!(payload.trace_id(), None);
    assert_eq!(payload.details(), Some(&json!({"field": "name"})));
}

#[given("a forbidden error code")]
fn a_forbidden_error_code() -> ErrorCode {
    ErrorCode::Forbidden
}

#[when("the adapter maps the code to an HTTP status")]
fn the_adapter_maps_the_code_to_http_status(code: ErrorCode) -> StatusCode {
    super::status_for(code)
}

#[then("the status is 403 Forbidden")]
fn the_status_is_403_forbidden(status: StatusCode) {
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[given("an internal error template")]
fn an_internal_error_template() -> (ErrorCode, &'static str) {
    (ErrorCode::InternalError, "boom")
}

#[when("the adapter redacts the client payload")]
fn the_adapter_redacts_the_client_payload(template: (ErrorCode, &'static str)) -> String {
    let (code, message) = template;
    let error = if let ErrorCode::InternalError = code {
        Error::internal(message)
    } else {
        Error::invalid_request(message)
    }
    .with_trace_id(TRACE_ID)
    .with_details(json!({"secret": true}));

    super::redact_if_internal(&error).message().to_owned()
}

#[then("clients see the generic internal error message")]
fn clients_see_the_generic_internal_error_message(message: String) {
    assert_eq!(message, "Internal server error");
}

#[test]
fn from_actix_error_is_redacted_internal_error() {
    use actix_web::error;

    let actix_err = error::ErrorBadRequest("boom");
    let err: Error = actix_err.into();

    assert_eq!(err.code(), ErrorCode::InternalError);
    assert_eq!(err.message(), "Internal server error");
    assert_eq!(err.trace_id(), None);
    assert_eq!(err.details(), None);
}
