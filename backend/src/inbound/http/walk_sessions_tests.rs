//! Tests for walk session HTTP handlers.

use super::*;
use crate::domain::ports::{
    FixtureCatalogueRepository, FixtureDescriptorRepository, FixtureLoginService,
    FixtureRouteAnnotationsCommand, FixtureRouteAnnotationsQuery, FixtureRouteSubmissionService,
    FixtureUserInterestsCommand, FixtureUserPreferencesCommand, FixtureUserPreferencesQuery,
    FixtureUserProfileQuery, FixtureUsersQuery,
};
use crate::inbound::http::state::HttpStatePorts;
use crate::inbound::http::users::LoginRequest;
use actix_web::http::StatusCode;
use actix_web::{App, test as actix_test, web};
use serde_json::Value;
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
                .service(create_walk_session),
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
        .find(|cookie| cookie.name() == "session")
        .expect("session cookie")
        .into_owned()
}

fn sample_walk_session_payload() -> Value {
    serde_json::json!({
        "id": "00000000-0000-0000-0000-000000000501",
        "routeId": "00000000-0000-0000-0000-000000000502",
        "startedAt": "2026-02-01T11:00:00Z",
        "endedAt": "2026-02-01T11:00:00Z",
        "primaryStats": [
            {"kind": "distance", "value": 1234.0},
            {"kind": "duration", "value": 456.0}
        ],
        "secondaryStats": [
            {"kind": "energy", "value": 120.0, "unit": "kcal"}
        ],
        "highlightedPoiIds": [
            "00000000-0000-0000-0000-000000000503"
        ]
    })
}

#[actix_web::test]
async fn create_walk_session_returns_stable_session_id() {
    let app = actix_test::init_service(test_app()).await;
    let cookie = login_and_get_cookie(&app).await;

    let request = actix_test::TestRequest::post()
        .uri("/api/v1/walk-sessions")
        .cookie(cookie)
        .set_json(sample_walk_session_payload())
        .to_request();
    let response = actix_test::call_service(&app, request).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = actix_test::read_body_json(response).await;
    assert_eq!(
        body.get("sessionId").and_then(Value::as_str),
        Some("00000000-0000-0000-0000-000000000501")
    );
    assert!(body.get("completionSummary").is_some());
}

#[actix_web::test]
async fn create_walk_session_rejects_invalid_route_id() {
    let app = actix_test::init_service(test_app()).await;
    let cookie = login_and_get_cookie(&app).await;

    let mut payload = sample_walk_session_payload();
    payload["routeId"] = Value::String("not-a-uuid".to_owned());

    let request = actix_test::TestRequest::post()
        .uri("/api/v1/walk-sessions")
        .cookie(cookie)
        .set_json(payload)
        .to_request();
    let response = actix_test::call_service(&app, request).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn create_walk_session_rejects_invalid_primary_stat_kind() {
    let app = actix_test::init_service(test_app()).await;
    let cookie = login_and_get_cookie(&app).await;

    let mut payload = sample_walk_session_payload();
    payload["primaryStats"][0]["kind"] = Value::String("pace".to_owned());

    let request = actix_test::TestRequest::post()
        .uri("/api/v1/walk-sessions")
        .cookie(cookie)
        .set_json(payload)
        .to_request();
    let response = actix_test::call_service(&app, request).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn walk_session_endpoint_requires_authenticated_session() {
    let app = actix_test::init_service(test_app()).await;

    let response = actix_test::call_service(
        &app,
        actix_test::TestRequest::post()
            .uri("/api/v1/walk-sessions")
            .set_json(sample_walk_session_payload())
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
