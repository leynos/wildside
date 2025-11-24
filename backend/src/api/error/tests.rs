//! Mapping domain errors to HTTP responses.

use super::*;
use crate::domain::{DomainError, ErrorCode, TRACE_ID_HEADER};
use crate::middleware::trace::TraceId;
use actix_web::body::to_bytes;
use actix_web::http::StatusCode;
use rstest::{fixture, rstest};
use serde_json::json;

#[fixture]
fn invalid_request() -> DomainError {
    DomainError::invalid_request("bad payload").with_details(json!({"field": "name"}))
}

#[fixture]
fn internal_failure() -> DomainError {
    DomainError::internal("boom").with_details(json!({"secret": "x"}))
}

#[rstest]
fn status_code_matches_error_code() {
    let cases = [
        (DomainError::invalid_request("bad"), StatusCode::BAD_REQUEST),
        (
            DomainError::unauthorized("no auth"),
            StatusCode::UNAUTHORIZED,
        ),
        (DomainError::forbidden("denied"), StatusCode::FORBIDDEN),
        (DomainError::not_found("missing"), StatusCode::NOT_FOUND),
        (
            DomainError::internal("boom"),
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
    ];
    for (domain_err, status) in cases {
        let api_err = ApiError::from(domain_err);
        assert_eq!(api_err.to_status_code(), status);
    }
}

async fn render_response(error: ApiError) -> (StatusCode, ApiError, Option<String>) {
    let response = error.error_response();
    let status = response.status();
    let trace = response
        .headers()
        .get(TRACE_ID_HEADER)
        .map(|v| v.to_str().expect("header is ascii").to_owned());
    let bytes = to_bytes(response.into_body()).await.expect("read body");
    let payload: ApiError = serde_json::from_slice(&bytes).expect("parse body");
    (status, payload, trace)
}

#[rstest]
#[tokio::test]
async fn error_response_captures_trace_id(invalid_request: DomainError) {
    let trace_id: TraceId = "00000000-0000-0000-0000-000000000001"
        .parse()
        .expect("valid trace id");
    let (status, body, header) = TraceId::scope(trace_id, async move {
        render_response(ApiError::from(invalid_request)).await
    })
    .await;

    let expected = trace_id.to_string();
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(header.as_deref(), Some(expected.as_str()));
    assert_eq!(body.message(), "bad payload");
    assert_eq!(body.code(), ErrorCode::InvalidRequest);
    assert_eq!(body.details(), Some(&json!({"field": "name"})));
}

#[rstest]
#[tokio::test]
async fn internal_errors_are_redacted(internal_failure: DomainError) {
    let trace_id: TraceId = "00000000-0000-0000-0000-000000000002"
        .parse()
        .expect("valid trace id");
    let (status, body, header) = TraceId::scope(trace_id, async move {
        render_response(ApiError::from(internal_failure)).await
    })
    .await;

    let expected = trace_id.to_string();
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(header.as_deref(), Some(expected.as_str()));
    assert_eq!(body.message(), "Internal server error");
    assert!(body.details().is_none());
}

#[rstest]
fn actix_errors_map_to_internal() {
    let source = actix_web::error::ErrorBadRequest("bad");
    let api_err = ApiError::from(source);
    assert_eq!(api_err.code(), ErrorCode::InternalError);
    assert_eq!(api_err.message(), "Internal server error");
}

#[rstest]
fn mapping_domain_error_preserves_fields() {
    let api_error = ApiError::from(DomainError::invalid_request("oops"));
    assert_eq!(api_error.code(), ErrorCode::InvalidRequest);
    assert_eq!(api_error.message(), "oops");
}
