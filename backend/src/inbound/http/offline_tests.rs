//! Tests for offline bundle HTTP handlers.

use super::*;
use crate::domain::ports::{
    FixtureCatalogueRepository, FixtureDescriptorRepository, FixtureLoginService,
    FixtureRouteAnnotationsCommand, FixtureRouteAnnotationsQuery, FixtureRouteSubmissionService,
    FixtureUserInterestsCommand, FixtureUserPreferencesCommand, FixtureUserPreferencesQuery,
    FixtureUserProfileQuery, FixtureUsersQuery,
};
use crate::inbound::http::idempotency::IDEMPOTENCY_KEY_HEADER;
use crate::inbound::http::state::HttpStatePorts;
use crate::inbound::http::users::LoginRequest;
use actix_web::http::StatusCode;
use actix_web::{App, test as actix_test, web};
use rstest::rstest;
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
                .service(list_offline_bundles)
                .service(upsert_offline_bundle)
                .service(delete_offline_bundle),
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

fn sample_bundle_payload() -> Value {
    serde_json::json!({
        "id": "00000000-0000-0000-0000-000000000101",
        "deviceId": "ios-iphone-15",
        "kind": "route",
        "routeId": "00000000-0000-0000-0000-000000000202",
        "regionId": null,
        "bounds": {
            "minLng": -3.2,
            "minLat": 55.9,
            "maxLng": -3.0,
            "maxLat": 56.0
        },
        "zoomRange": {
            "minZoom": 11,
            "maxZoom": 15
        },
        "estimatedSizeBytes": 4096,
        "createdAt": "2026-02-01T10:00:00Z",
        "updatedAt": "2026-02-01T10:00:00Z",
        "status": "queued",
        "progress": 0.0
    })
}

async fn setup_authenticated_test() -> (
    impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    actix_web::cookie::Cookie<'static>,
) {
    let app = actix_test::init_service(test_app()).await;
    let cookie = login_and_get_cookie(&app).await;
    (app, cookie)
}

fn assert_bundle_id(body: &Value, expected: &str) {
    assert_eq!(body.get("bundleId").and_then(Value::as_str), Some(expected));
}

#[actix_web::test]
async fn list_offline_bundles_returns_empty_list_for_fixture_query() {
    let (app, cookie) = setup_authenticated_test().await;

    let request = actix_test::TestRequest::get()
        .uri("/api/v1/offline/bundles?deviceId=ios-iphone-15")
        .cookie(cookie)
        .to_request();
    let response = actix_test::call_service(&app, request).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = actix_test::read_body_json(response).await;
    let bundles = body
        .get("bundles")
        .and_then(Value::as_array)
        .expect("bundles array");
    assert!(bundles.is_empty());
}

#[actix_web::test]
async fn list_offline_bundles_rejects_missing_device_id() {
    let (app, cookie) = setup_authenticated_test().await;

    let request = actix_test::TestRequest::get()
        .uri("/api/v1/offline/bundles")
        .cookie(cookie)
        .to_request();
    let response = actix_test::call_service(&app, request).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn upsert_offline_bundle_returns_stable_bundle_id() {
    let (app, cookie) = setup_authenticated_test().await;

    let request = actix_test::TestRequest::post()
        .uri("/api/v1/offline/bundles")
        .cookie(cookie)
        .set_json(sample_bundle_payload())
        .to_request();
    let response = actix_test::call_service(&app, request).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = actix_test::read_body_json(response).await;
    assert_bundle_id(&body, "00000000-0000-0000-0000-000000000101");
}

#[rstest]
#[case("not-a-uuid")]
#[case("")]
#[actix_web::test]
async fn upsert_offline_bundle_rejects_invalid_idempotency_key(#[case] invalid_key: &str) {
    let (app, cookie) = setup_authenticated_test().await;

    let request = actix_test::TestRequest::post()
        .uri("/api/v1/offline/bundles")
        .cookie(cookie)
        .insert_header((IDEMPOTENCY_KEY_HEADER, invalid_key))
        .set_json(sample_bundle_payload())
        .to_request();
    let response = actix_test::call_service(&app, request).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn delete_offline_bundle_returns_requested_id() {
    let (app, cookie) = setup_authenticated_test().await;

    let request = actix_test::TestRequest::delete()
        .uri("/api/v1/offline/bundles/00000000-0000-0000-0000-000000000303")
        .cookie(cookie)
        .to_request();
    let response = actix_test::call_service(&app, request).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = actix_test::read_body_json(response).await;
    assert_bundle_id(&body, "00000000-0000-0000-0000-000000000303");
}

#[actix_web::test]
async fn delete_offline_bundle_rejects_invalid_bundle_id() {
    let (app, cookie) = setup_authenticated_test().await;

    let request = actix_test::TestRequest::delete()
        .uri("/api/v1/offline/bundles/not-a-uuid")
        .cookie(cookie)
        .to_request();
    let response = actix_test::call_service(&app, request).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn offline_endpoints_require_authenticated_session() {
    let app = actix_test::init_service(test_app()).await;

    let response = actix_test::call_service(
        &app,
        actix_test::TestRequest::get()
            .uri("/api/v1/offline/bundles?deviceId=ios-iphone-15")
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
