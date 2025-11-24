//! Tests for mapping domain errors onto HTTP responses.

use super::*;
use crate::domain::ErrorValidationError;
use crate::middleware::trace::TraceId;
use actix_web::{body::to_bytes, http::StatusCode, ResponseError};
use pg_embedded_setup_unpriv::{test_support::test_cluster, TestCluster};
use rstest::{fixture, rstest};
use rstest_bdd_macros::{given, then, when};
use serde_json::json;

#[fixture]
fn domain_error() -> DomainError {
    DomainError::invalid_request("bad").with_details(json!({ "field": "name" }))
}

#[rstest]
fn from_domain_preserves_details(domain_error: DomainError) {
    let api_error = ApiError::from(domain_error);

    assert_eq!(api_error.code(), ErrorCode::InvalidRequest);
    assert_eq!(api_error.message(), "bad");
    assert_eq!(api_error.details(), Some(&json!({ "field": "name" })));
}

#[rstest]
#[actix_web::test]
async fn response_error_redacts_internal_details() {
    let api_error =
        ApiError::from(DomainError::internal("boom").with_details(json!({ "secret": "x" })));

    let response = api_error.error_response();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert!(response.headers().get(TRACE_ID_HEADER).is_none());

    let bytes = to_bytes(response.into_body())
        .await
        .expect("response body to bytes");
    let payload: ApiError = serde_json::from_slice(&bytes).expect("payload deserialises");

    assert_eq!(payload.message(), "Internal server error");
    assert!(payload.details().is_none());
}

#[rstest]
fn try_from_rejects_empty_trace_id() {
    let dto = ApiErrorDto {
        code: ErrorCode::Unauthorized,
        message: "bad".to_string(),
        trace_id: Some("   ".to_string()),
        details: None,
    };

    let result = ApiError::try_from(dto);
    assert!(matches!(result, Err(ErrorValidationError::EmptyTraceId)));
}

#[rstest]
#[actix_web::test]
async fn includes_trace_id_when_scoped() {
    let trace_id: TraceId = "00000000-0000-0000-0000-000000000000"
        .parse()
        .expect("valid UUID literal");

    let api_error = TraceId::scope(trace_id, async move {
        ApiError::from(DomainError::not_found("missing"))
    })
    .await;

    let response = api_error.error_response();
    let header = response
        .headers()
        .get(TRACE_ID_HEADER)
        .expect("trace header present");
    let header_value = header.to_str().expect("trace id is ASCII");
    assert_eq!(header_value, trace_id.to_string());

    let body: ApiError = serde_json::from_slice(&to_bytes(response.into_body()).await.unwrap())
        .expect("payload deserialises");
    assert_eq!(body.trace_id(), Some(trace_id.to_string().as_str()));
}

#[fixture]
fn running_cluster(#[from(test_cluster)] test_cluster: TestCluster) -> TestCluster {
    test_cluster
}

#[derive(Debug, Clone)]
enum MappedError {
    Success(ApiError),
}

#[given("a running embedded postgres cluster")]
fn a_running_embedded_postgres_cluster(running_cluster: TestCluster) -> TestCluster {
    running_cluster
}

#[when("an invalid request is mapped with cluster metadata")]
fn an_invalid_request_is_mapped_with_cluster_metadata(test_cluster: TestCluster) -> MappedError {
    let port = test_cluster.connection().metadata().port();
    let api_error = ApiError::from(
        DomainError::invalid_request("bad input").with_details(json!({ "pg_port": port })),
    );
    MappedError::Success(api_error)
}

#[then("the api error surfaces the metadata")]
fn the_api_error_surfaces_the_metadata(mapped: MappedError) {
    let MappedError::Success(error) = mapped;
    assert_eq!(error.code(), ErrorCode::InvalidRequest);
    let details = error.details().expect("details present");
    assert!(details.get("pg_port").is_some());
}

#[rstest]
fn mapping_preserves_metadata_happy_path(running_cluster: TestCluster) {
    let mapped = an_invalid_request_is_mapped_with_cluster_metadata(running_cluster);
    the_api_error_surfaces_the_metadata(mapped);
}
