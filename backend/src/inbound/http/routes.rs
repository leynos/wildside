//! Routes API handlers.
//!
//! ```text
//! POST /api/v1/routes  Submit a route generation request
//! ```
//!
//! Supports idempotent request submission via the `Idempotency-Key` header.

use actix_web::http::header::HeaderMap;
use actix_web::{HttpRequest, HttpResponse, post, web};
use serde::{Deserialize, Serialize};

use crate::domain::ports::{RouteSubmissionRequest, RouteSubmissionStatus};
use crate::domain::{Error, IdempotencyKey, IdempotencyKeyValidationError};
use crate::inbound::http::ApiResult;
use crate::inbound::http::session::SessionContext;
use crate::inbound::http::state::HttpState;

/// HTTP header name for idempotency keys.
pub const IDEMPOTENCY_KEY_HEADER: &str = "Idempotency-Key";

/// Route generation request body.
///
/// The structure of route requests is intentionally flexible during early
/// development. The payload is validated by downstream services.
#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RouteRequest {
    /// Origin location identifier or coordinates.
    pub origin: serde_json::Value,
    /// Destination location identifier or coordinates.
    pub destination: serde_json::Value,
    /// Optional route preferences.
    #[serde(default)]
    pub preferences: Option<serde_json::Value>,
}

/// Route submission response.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RouteResponse {
    /// Unique identifier for this route request.
    pub request_id: String,
    /// Status of the submission.
    pub status: String,
}

/// Extract the idempotency key from request headers.
fn extract_idempotency_key(
    headers: &HeaderMap,
) -> Result<Option<IdempotencyKey>, IdempotencyKeyValidationError> {
    let Some(header_value) = headers.get(IDEMPOTENCY_KEY_HEADER) else {
        return Ok(None);
    };

    let key_str = header_value
        .to_str()
        .map_err(|_| IdempotencyKeyValidationError::InvalidKey)?;

    IdempotencyKey::new(key_str).map(Some)
}

/// Map idempotency key validation errors to domain errors.
fn map_idempotency_key_error(err: IdempotencyKeyValidationError) -> Error {
    match err {
        IdempotencyKeyValidationError::EmptyKey => {
            Error::invalid_request("Idempotency-Key header must not be empty")
        }
        IdempotencyKeyValidationError::InvalidKey => {
            Error::invalid_request("Idempotency-Key header must be a valid UUID")
        }
    }
}

/// Submit a route generation request.
///
/// # Idempotency
///
/// Clients may provide an `Idempotency-Key` header (UUID format) for safe
/// retries. When a key is provided:
///
/// - First request: Returns `202 Accepted` with a new `requestId`.
/// - Duplicate with same payload: Returns `202 Accepted` with the original
///   `requestId` (status: `replayed`).
/// - Duplicate with different payload: Returns `409 Conflict`.
///
/// # Errors
///
/// - `400 Bad Request`: Invalid idempotency key format or request body.
/// - `401 Unauthorized`: No valid session.
/// - `409 Conflict`: Idempotency key reused with different payload.
/// - `503 Service Unavailable`: Backend services unavailable.
#[utoipa::path(
    post,
    path = "/api/v1/routes",
    request_body = RouteRequest,
    responses(
        (status = 202, description = "Route request accepted", body = RouteResponse),
        (status = 400, description = "Invalid request", body = crate::inbound::http::schemas::ErrorSchema),
        (status = 401, description = "Unauthorised", body = crate::inbound::http::schemas::ErrorSchema),
        (status = 409, description = "Idempotency key conflict", body = crate::inbound::http::schemas::ErrorSchema),
        (status = 503, description = "Service unavailable", body = crate::inbound::http::schemas::ErrorSchema)
    ),
    params(
        ("Idempotency-Key" = Option<String>, Header, description = "UUID for idempotent request submission")
    ),
    tags = ["routes"],
    operation_id = "submitRoute"
)]
#[post("/routes")]
pub async fn submit_route(
    state: web::Data<HttpState>,
    session: SessionContext,
    request: HttpRequest,
    payload: web::Json<RouteRequest>,
) -> ApiResult<HttpResponse> {
    let user_id = session.require_user_id()?;

    // Extract and validate idempotency key from headers.
    let idempotency_key =
        extract_idempotency_key(request.headers()).map_err(map_idempotency_key_error)?;

    // Convert request body to JSON value for hashing.
    let payload_value = serde_json::to_value(payload.into_inner())
        .map_err(|err| Error::internal(format!("failed to serialize request: {err}")))?;

    // Build submission request.
    let submission_request = RouteSubmissionRequest {
        idempotency_key,
        user_id,
        payload: payload_value,
    };

    // Delegate to service.
    let response = state.route_submission.submit(submission_request).await?;

    // Build HTTP response.
    let status_str = match response.status {
        RouteSubmissionStatus::Accepted => "accepted",
        RouteSubmissionStatus::Replayed => "replayed",
    };

    let body = RouteResponse {
        request_id: response.request_id.to_string(),
        status: status_str.to_owned(),
    };

    Ok(HttpResponse::Accepted().json(body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ports::{
        FixtureLoginService, FixtureRouteSubmissionService, FixtureUserInterestsCommand,
        FixtureUserProfileQuery, FixtureUsersQuery,
    };
    use crate::inbound::http::state::HttpStatePorts;
    use crate::inbound::http::users::LoginRequest;
    use actix_web::http::StatusCode;
    use actix_web::{App, test as actix_test, web};
    use rstest::rstest;
    use serde_json::{Value, json};
    use std::sync::Arc;

    fn test_app() -> App<
        impl actix_web::dev::ServiceFactory<
            actix_web::dev::ServiceRequest,
            Config = (),
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
            InitError = (),
        >,
    > {
        let state = HttpState::new(HttpStatePorts {
            login: Arc::new(FixtureLoginService),
            users: Arc::new(FixtureUsersQuery),
            profile: Arc::new(FixtureUserProfileQuery),
            interests: Arc::new(FixtureUserInterestsCommand),
            route_submission: Arc::new(FixtureRouteSubmissionService),
        });
        App::new()
            .app_data(web::Data::new(state))
            .wrap(crate::inbound::http::test_utils::test_session_middleware())
            .service(
                web::scope("/api/v1")
                    .service(crate::inbound::http::users::login)
                    .service(submit_route),
            )
    }

    async fn login_and_get_cookie(
        app: &impl actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
    ) -> actix_web::cookie::Cookie<'static> {
        let login_req = actix_test::TestRequest::post()
            .uri("/api/v1/login")
            .set_json(&LoginRequest {
                username: "admin".into(),
                password: "password".into(),
            })
            .to_request();
        let login_res = actix_test::call_service(app, login_req).await;
        assert!(login_res.status().is_success());
        login_res
            .response()
            .cookies()
            .find(|c| c.name() == "session")
            .expect("session cookie")
            .into_owned()
    }

    #[actix_web::test]
    async fn submit_route_accepts_request_without_idempotency_key() {
        let app = actix_test::init_service(test_app()).await;
        let cookie = login_and_get_cookie(&app).await;

        let request = actix_test::TestRequest::post()
            .uri("/api/v1/routes")
            .cookie(cookie)
            .set_json(json!({
                "origin": {"lat": 51.5, "lng": -0.1},
                "destination": {"lat": 48.8, "lng": 2.3}
            }))
            .to_request();

        let response = actix_test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);

        let body: Value = actix_test::read_body_json(response).await;
        assert!(body.get("requestId").is_some());
        assert_eq!(body.get("status").and_then(Value::as_str), Some("accepted"));
    }

    #[actix_web::test]
    async fn submit_route_accepts_request_with_valid_idempotency_key() {
        let app = actix_test::init_service(test_app()).await;
        let cookie = login_and_get_cookie(&app).await;

        let request = actix_test::TestRequest::post()
            .uri("/api/v1/routes")
            .cookie(cookie)
            .insert_header((
                IDEMPOTENCY_KEY_HEADER,
                "550e8400-e29b-41d4-a716-446655440000",
            ))
            .set_json(json!({
                "origin": {"lat": 51.5, "lng": -0.1},
                "destination": {"lat": 48.8, "lng": 2.3}
            }))
            .to_request();

        let response = actix_test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    #[rstest]
    #[case("not-a-uuid")]
    #[case("550e8400")]
    #[case("")]
    #[actix_web::test]
    async fn submit_route_rejects_invalid_idempotency_key(#[case] invalid_key: &str) {
        let app = actix_test::init_service(test_app()).await;
        let cookie = login_and_get_cookie(&app).await;

        let request = actix_test::TestRequest::post()
            .uri("/api/v1/routes")
            .cookie(cookie)
            .insert_header((IDEMPOTENCY_KEY_HEADER, invalid_key))
            .set_json(json!({
                "origin": {"lat": 51.5, "lng": -0.1},
                "destination": {"lat": 48.8, "lng": 2.3}
            }))
            .to_request();

        let response = actix_test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn submit_route_rejects_without_session() {
        let app = actix_test::init_service(test_app()).await;

        let request = actix_test::TestRequest::post()
            .uri("/api/v1/routes")
            .set_json(json!({
                "origin": {"lat": 51.5, "lng": -0.1},
                "destination": {"lat": 48.8, "lng": 2.3}
            }))
            .to_request();

        let response = actix_test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
