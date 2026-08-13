//! Routes API handlers.
//!
//! ```text
//! POST /api/v1/routes  Submit a route generation request
//! ```
//!
//! Supports idempotent request submission via the `Idempotency-Key` header.

use actix_web::{HttpRequest, HttpResponse, post, web};
use serde::Serialize;

use crate::domain::ports::{RouteSubmissionRequest, RouteSubmissionStatus};
use crate::inbound::http::ApiResult;
use crate::inbound::http::idempotency::{extract_idempotency_key, map_idempotency_key_error};
use crate::inbound::http::session::SessionContext;
use crate::inbound::http::state::HttpState;

#[path = "routes/request.rs"]
mod request;
pub use request::{RouteCoordinatesDto, RouteLocationDto, RouteRequest};

/// Route submission response.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RouteResponse {
    /// Unique identifier for this route request.
    pub request_id: String,
    /// Status of the submission.
    pub status: String,
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
        (status = 401, description = "Unauthorized", body = crate::inbound::http::schemas::ErrorSchema),
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

    let idempotency_key =
        extract_idempotency_key(request.headers()).map_err(map_idempotency_key_error)?;

    let route_payload = payload.into_inner().try_into()?;
    let submission_request = RouteSubmissionRequest {
        idempotency_key,
        user_id,
        payload: route_payload,
    };

    let response = state.route_submission.submit(submission_request).await?;

    let status_str = match response.status {
        RouteSubmissionStatus::Accepted => "accepted",
        RouteSubmissionStatus::Replayed => "replayed",
    };

    let body = RouteResponse {
        request_id: response.request_id.to_string(),
        status: status_str.to_string(),
    };

    Ok(HttpResponse::Accepted().json(body))
}

#[cfg(test)]
mod tests {
    //! Regression coverage for this module.
    use super::*;
    use crate::domain::ports::{
        FixtureCatalogueRepository, FixtureDescriptorRepository, FixtureLoginService,
        FixtureRouteAnnotationsCommand, FixtureRouteAnnotationsQuery,
        FixtureRouteSubmissionService, FixtureUserInterestsCommand, FixtureUserPreferencesCommand,
        FixtureUserPreferencesQuery, FixtureUserProfileQuery, FixtureUsersQuery,
    };
    use crate::inbound::http::idempotency::IDEMPOTENCY_KEY_HEADER;
    use crate::inbound::http::state::HttpStatePorts;
    use crate::inbound::http::users::LoginRequest;
    use actix_web::http::StatusCode;
    use actix_web::{App, test as actix_test, web};
    use rstest::rstest;
    use serde_json::{Value, json};
    use std::{error::Error as StdError, io, sync::Arc};

    type TestResult<T = ()> = Result<T, Box<dyn StdError>>;

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
            preferences: Arc::new(FixtureUserPreferencesCommand),
            preferences_query: Arc::new(FixtureUserPreferencesQuery),
            route_annotations: Arc::new(FixtureRouteAnnotationsCommand),
            route_annotations_query: Arc::new(FixtureRouteAnnotationsQuery),
            route_submission: Arc::new(FixtureRouteSubmissionService),
            catalogue: Arc::new(FixtureCatalogueRepository),
            descriptors: Arc::new(FixtureDescriptorRepository),
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
    ) -> TestResult<actix_web::cookie::Cookie<'static>> {
        let login_req = actix_test::TestRequest::post()
            .uri("/api/v1/login")
            .set_json(&LoginRequest {
                username: "admin".into(),
                password: "password".into(),
            })
            .to_request();
        let login_res = actix_test::call_service(app, login_req).await;
        assert!(login_res.status().is_success());
        Ok(login_res
            .response()
            .cookies()
            .find(|c| c.name() == "session")
            .ok_or_else(|| io::Error::other("session cookie"))?
            .into_owned())
    }

    #[actix_web::test]
    async fn submit_route_accepts_request_without_idempotency_key() -> TestResult {
        let app = actix_test::init_service(test_app()).await;
        let cookie = login_and_get_cookie(&app).await?;

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
        Ok(())
    }

    #[actix_web::test]
    async fn submit_route_accepts_request_with_valid_idempotency_key() -> TestResult {
        let app = actix_test::init_service(test_app()).await;
        let cookie = login_and_get_cookie(&app).await?;

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
        Ok(())
    }

    #[rstest]
    #[case("not-a-uuid")]
    #[case("550e8400")]
    #[case("")]
    #[actix_web::test]
    async fn submit_route_rejects_invalid_idempotency_key(#[case] invalid_key: &str) -> TestResult {
        let app = actix_test::init_service(test_app()).await;
        let cookie = login_and_get_cookie(&app).await?;

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
        Ok(())
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

    #[rstest]
    #[case::coordinates(json!({"origin":{"lat":51.5,"lng":-0.1},"destination":{"lat":48.8,"lng":2.3}}), true)]
    #[case::identifiers(json!({"origin":"saved:home","destination":"poi:work"}), true)]
    #[case::minimum_coordinates(json!({"origin":{"lat":-90.0,"lng":-180.0},"destination":{"lat":48.8,"lng":2.3}}), true)]
    #[case::maximum_coordinates(json!({"origin":{"lat":90.0,"lng":180.0},"destination":{"lat":48.8,"lng":2.3}}), true)]
    #[case::boolean_origin(json!({"origin":true,"destination":"poi:work"}), false)]
    #[case::array_destination(json!({"origin":"saved:home","destination":[48.8,2.3]}), false)]
    #[case::null_origin(json!({"origin":null,"destination":"poi:work"}), false)]
    #[case::null_destination(json!({"origin":"saved:home","destination":null}), false)]
    #[case::null_preferences(json!({"origin":"saved:home","destination":"poi:work","preferences":null}), false)]
    #[case::latitude_too_low(json!({"origin":{"lat":-90.1,"lng":0.0},"destination":"poi:work"}), false)]
    #[case::latitude_too_high(json!({"origin":{"lat":90.1,"lng":0.0},"destination":"poi:work"}), false)]
    #[case::longitude_too_low(json!({"origin":{"lat":0.0,"lng":-180.1},"destination":"poi:work"}), false)]
    #[case::longitude_too_high(json!({"origin":{"lat":0.0,"lng":180.1},"destination":"poi:work"}), false)]
    #[case::unknown_top_level(json!({"origin":"saved:home","destination":"poi:work","extra":true}), false)]
    #[case::unknown_location_field(json!({"origin":{"lat":51.5,"lng":-0.1,"extra":true},"destination":"poi:work"}), false)]
    #[case::unknown_preferences_field(json!({"origin":"saved:home","destination":"poi:work","preferences":{"extra":true}}), false)]
    fn route_request_validates_documented_shapes(
        #[case] payload: Value,
        #[case] should_accept: bool,
    ) {
        let result = serde_json::from_value::<RouteRequest>(payload);

        assert_eq!(result.is_ok(), should_accept);
    }

    #[rstest]
    #[case::latitude(-90.1, 0.0)]
    #[case::longitude(0.0, 180.1)]
    fn route_coordinate_dto_conversion_rejects_invalid_wgs84_values(
        #[case] lat: f64,
        #[case] lng: f64,
    ) {
        let result =
            crate::domain::ports::RouteCoordinates::try_from(RouteCoordinatesDto { lat, lng });

        assert!(result.is_err());
    }

    #[test]
    fn route_request_omits_absent_preferences_when_serialized() {
        let request = RouteRequest {
            origin: RouteLocationDto::Identifier("saved:home".to_owned()),
            destination: RouteLocationDto::Identifier("poi:work".to_owned()),
            preferences: None,
        };

        let value = serde_json::to_value(request).expect("route request should serialize");

        assert!(value.get("preferences").is_none());
    }
}
