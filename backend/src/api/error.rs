//! Adapter-level error mapping from domain errors to HTTP responses.
//!
//! Keep the domain error type free of framework concerns; this module
//! translates it into Actix responses and enriches the payload with the
//! current trace identifier when available.

use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use thiserror::Error;
use tracing::error;

use crate::{
    domain::{Error as DomainError, ErrorCode, TRACE_ID_HEADER},
    middleware::trace::TraceId,
};

/// HTTP-facing error wrapper that knows how to render a JSON payload.
#[derive(Debug, Clone, Error)]
#[error("{0}")]
pub struct ApiError(pub DomainError);

impl ApiError {
    fn http_status(&self) -> StatusCode {
        match self.0.code() {
            ErrorCode::InvalidRequest => StatusCode::BAD_REQUEST,
            ErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrorCode::Forbidden => StatusCode::FORBIDDEN,
            ErrorCode::NotFound => StatusCode::NOT_FOUND,
            ErrorCode::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn payload(&self) -> DomainError {
        let mut payload = self.0.clone();
        if payload.trace_id().is_none() {
            if let Some(trace_id) = TraceId::current() {
                match payload.with_optional_trace_id(Some(trace_id.to_string())) {
                    Ok(updated) => payload = updated,
                    Err(err) => {
                        error!(%err, "failed to attach trace identifier to error payload");
                    }
                }
            }
        }
        if matches!(payload.code(), ErrorCode::InternalError) {
            return payload.redacted_for_clients();
        }
        payload
    }
}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        self.http_status()
    }

    fn error_response(&self) -> HttpResponse {
        let payload = self.payload();
        let mut builder = HttpResponse::build(self.http_status());

        if let Some(trace_id) = payload.trace_id() {
            builder.insert_header((TRACE_ID_HEADER, trace_id.to_owned()));
        }

        builder.json(payload)
    }
}

impl From<DomainError> for ApiError {
    fn from(value: DomainError) -> Self {
        Self(value)
    }
}

impl From<actix_web::Error> for ApiError {
    fn from(err: actix_web::Error) -> Self {
        error!(error = %err, "actix error promoted to API error");
        Self(DomainError::internal("Internal server error"))
    }
}

/// Convenient alias for handlers returning the adapter error wrapper.
pub type ApiResult<T> = Result<T, ApiError>;

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::body::to_bytes;
    use actix_web::http::StatusCode;
    use pg_embedded_setup_unpriv::test_support::test_cluster;
    use pg_embedded_setup_unpriv::TestCluster;
    use rstest::{fixture, rstest};
    use rstest_bdd_macros::{given, then, when};
    use serde_json::json;

    const TRACE_ID: &str = "11111111-1111-1111-1111-111111111111";

    #[fixture]
    fn trace_id() -> TraceId {
        TRACE_ID
            .parse()
            .expect("fixture provides a valid UUID trace identifier")
    }

    #[rstest]
    fn maps_error_codes_to_status() {
        let cases = [
            (
                DomainError::invalid_request("bad input"),
                StatusCode::BAD_REQUEST,
            ),
            (
                DomainError::unauthorized("no session"),
                StatusCode::UNAUTHORIZED,
            ),
            (DomainError::forbidden("nope"), StatusCode::FORBIDDEN),
            (DomainError::not_found("missing"), StatusCode::NOT_FOUND),
            (
                DomainError::internal("boom"),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        ];

        for (error, expected) in cases {
            let api_error = ApiError::from(error);
            assert_eq!(api_error.http_status(), expected);
        }
    }

    #[rstest]
    async fn attaches_trace_and_redacts_internal_errors(trace_id: TraceId) {
        let error = DomainError::internal("boom").with_details(json!({"secret": "keep-me"}));
        let response = TraceId::scope(
            trace_id,
            async move { ApiError::from(error).error_response() },
        )
        .await;

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let header = response
            .headers()
            .get(TRACE_ID_HEADER)
            .expect("trace id header is present")
            .to_str()
            .expect("trace id header is ascii");
        assert_eq!(header, TRACE_ID);

        let body = to_bytes(response.into_body()).await.expect("readable body");
        let payload: DomainError =
            serde_json::from_slice(&body).expect("error payload deserialises");
        assert_eq!(payload.message(), "Internal server error");
        assert!(payload.details().is_none());
        assert_eq!(payload.trace_id(), Some(TRACE_ID));
    }

    #[derive(Debug)]
    struct RenderedError {
        status: StatusCode,
        trace_id: String,
        message: String,
        details_present: bool,
    }

    #[given("an internal error with database context")]
    fn internal_error_with_database_context(
        #[from(test_cluster)] test_cluster: TestCluster,
    ) -> (ApiError, String) {
        let trace_id = TRACE_ID.to_owned();
        let metadata = test_cluster.connection().metadata();
        let details = json!({
            "databaseUrl": metadata.database_url("postgres"),
            "port": metadata.port()
        });
        let error = DomainError::internal("connection failed").with_details(details);
        (ApiError::from(error), trace_id)
    }

    #[when("the adapter renders the HTTP response")]
    async fn render_http_response(input: (ApiError, String)) -> RenderedError {
        let (error, trace_id) = input;
        let trace: TraceId = trace_id
            .parse()
            .expect("fixtures provide a valid trace identifier");
        let response = TraceId::scope(trace, async move { error.error_response() }).await;
        let status = response.status();
        let header = response
            .headers()
            .get(TRACE_ID_HEADER)
            .expect("trace header")
            .to_str()
            .expect("ascii header")
            .to_owned();
        let body = to_bytes(response.into_body()).await.expect("body bytes");
        let payload: DomainError = serde_json::from_slice(&body).expect("payload deserialises");
        RenderedError {
            status,
            trace_id: header,
            message: payload.message().to_owned(),
            details_present: payload.details().is_some(),
        }
    }

    #[then("the response is redacted but carries the trace id")]
    fn assert_redaction(rendered: RenderedError) {
        assert_eq!(rendered.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(rendered.trace_id, TRACE_ID);
        assert_eq!(rendered.message, "Internal server error");
        assert!(
            !rendered.details_present,
            "internal error details must be stripped from client payloads"
        );
    }

    #[rstest]
    async fn redaction_behavioural_scenario(
        #[from(internal_error_with_database_context)] context: (ApiError, String),
    ) {
        let rendered = render_http_response(context).await;
        assert_redaction(rendered);
    }
}
