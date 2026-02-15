//! Tests for users API handlers.

use super::*;
use crate::domain::ErrorCode;
use crate::domain::ports::{
    FixtureCatalogueRepository, FixtureDescriptorRepository, FixtureLoginService,
    FixtureRouteAnnotationsCommand, FixtureRouteAnnotationsQuery, FixtureRouteSubmissionService,
    FixtureUserInterestsCommand, FixtureUserPreferencesCommand, FixtureUserPreferencesQuery,
    FixtureUserProfileQuery, FixtureUsersQuery,
};
use crate::inbound::http::state::HttpStatePorts;
use actix_web::{App, test as actix_test, web};
use rstest::rstest;
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug)]
struct ValidationExpectation<'a> {
    message: &'a str,
    field: &'a str,
    code: &'a str,
    top_code: &'a str,
}

async fn assert_login_validation_error(
    username: &str,
    password: &str,
    expected: ValidationExpectation<'_>,
) {
    let app = actix_test::init_service(test_app()).await;

    let request = actix_test::TestRequest::post()
        .uri("/api/v1/login")
        .set_json(&LoginRequest {
            username: username.into(),
            password: password.into(),
        })
        .to_request();

    let response = actix_test::call_service(&app, request).await;
    assert_eq!(response.status(), actix_web::http::StatusCode::BAD_REQUEST);
    let body = actix_test::read_body(response).await;
    let value: Value = serde_json::from_slice(&body).expect("error payload");
    assert_eq!(
        value.get("message").and_then(Value::as_str),
        Some(expected.message)
    );
    assert_eq!(
        value.get("code").and_then(Value::as_str),
        Some(expected.top_code)
    );
    let details = value
        .get("details")
        .and_then(|v| v.as_object())
        .expect("details present");
    assert_eq!(
        details.get("field").and_then(Value::as_str),
        Some(expected.field)
    );
    assert_eq!(
        details.get("code").and_then(Value::as_str),
        Some(expected.code)
    );
}

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
                .service(login)
                .service(list_users)
                .service(current_user)
                .service(update_interests),
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

#[rstest]
#[case(
    "   ",
    "password",
    ValidationExpectation {
        message: "username must not be empty",
        field: "username",
        code: "empty_username",
        top_code: "invalid_request",
    }
)]
#[case(
    "admin",
    "",
    ValidationExpectation {
        message: "password must not be empty",
        field: "password",
        code: "empty_password",
        top_code: "invalid_request",
    }
)]
#[actix_web::test]
async fn login_rejects_invalid_credentials(
    #[case] username: &str,
    #[case] password: &str,
    #[case] expected: ValidationExpectation<'_>,
) {
    assert_login_validation_error(username, password, expected).await;
}

#[actix_web::test]
async fn login_rejects_wrong_credentials_with_unauthorised_status() {
    let app = actix_test::init_service(test_app()).await;
    let request = actix_test::TestRequest::post()
        .uri("/api/v1/login")
        .set_json(&LoginRequest {
            username: "admin".into(),
            password: "wrong-password".into(),
        })
        .to_request();

    let response = actix_test::call_service(&app, request).await;
    assert_eq!(response.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    let body = actix_test::read_body(response).await;
    let value: Value = serde_json::from_slice(&body).expect("error payload");
    assert_eq!(
        value.get("message").and_then(Value::as_str),
        Some("invalid credentials")
    );
    assert_eq!(
        value.get("code").and_then(Value::as_str),
        Some("unauthorized")
    );
}

#[actix_web::test]
async fn list_users_returns_camel_case_json() {
    let app = actix_test::init_service(test_app()).await;
    let cookie = login_and_get_cookie(&app).await;

    let users_req = actix_test::TestRequest::get()
        .uri("/api/v1/users")
        .cookie(cookie)
        .to_request();
    let users_res = actix_test::call_service(&app, users_req).await;
    assert!(users_res.status().is_success());
    let body = actix_test::read_body(users_res).await;
    let value: Value = serde_json::from_slice(&body).expect("response JSON");
    let first = &value.as_array().expect("array")[0];
    assert_eq!(
        first.get("displayName").and_then(Value::as_str),
        Some("Ada Lovelace")
    );
    assert!(first.get("display_name").is_none());
}

#[actix_web::test]
async fn list_users_rejects_without_session() {
    let app = actix_test::init_service(test_app()).await;
    let response = actix_test::call_service(
        &app,
        actix_test::TestRequest::get()
            .uri("/api/v1/users")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), actix_web::http::StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn update_interests_rejects_too_many_ids() {
    let app = actix_test::init_service(test_app()).await;
    let cookie = login_and_get_cookie(&app).await;
    let payload = InterestsRequest {
        interest_theme_ids: vec![
            "3fa85f64-5717-4562-b3fc-2c963f66afa6".to_owned();
            INTEREST_THEME_IDS_MAX + 1
        ],
    };

    let request = actix_test::TestRequest::put()
        .uri("/api/v1/users/me/interests")
        .cookie(cookie)
        .set_json(payload)
        .to_request();
    let response = actix_test::call_service(&app, request).await;

    assert_eq!(response.status(), actix_web::http::StatusCode::BAD_REQUEST);
    let body = actix_test::read_body(response).await;
    let value: Value = serde_json::from_slice(&body).expect("error payload");
    assert_eq!(
        value.get("message").and_then(Value::as_str),
        Some("interest theme ids must contain at most 100 items")
    );
    assert_eq!(
        value.get("code").and_then(Value::as_str),
        Some("invalid_request")
    );
    let details = value
        .get("details")
        .and_then(|val| val.as_object())
        .expect("details present");
    assert_eq!(
        details.get("code").and_then(Value::as_str),
        Some("too_many_interest_theme_ids")
    );
    assert_eq!(
        details.get("field").and_then(Value::as_str),
        Some("interestThemeIds")
    );
    assert_eq!(
        details.get("max").and_then(Value::as_u64),
        Some(INTEREST_THEME_IDS_MAX as u64)
    );
    assert_eq!(
        details.get("count").and_then(Value::as_u64),
        Some((INTEREST_THEME_IDS_MAX + 1) as u64)
    );
}

#[rstest]
#[case("", "empty_interest_theme_id", "interest theme id must not be empty")]
#[case(
    "not-a-uuid",
    "invalid_interest_theme_id",
    "interest theme id must be a valid UUID"
)]
fn interests_request_validation_rejects_invalid_ids(
    #[case] value: &str,
    #[case] expected_code: &str,
    #[case] expected_message: &str,
) {
    let payload = InterestsRequest {
        interest_theme_ids: vec![value.to_owned()],
    };

    let err = parse_interest_theme_ids(payload).expect_err("invalid interest theme id");
    let api_error = map_interests_request_error(err);

    assert_eq!(api_error.code(), ErrorCode::InvalidRequest);
    assert_eq!(api_error.message(), expected_message);
    let details = api_error
        .details()
        .and_then(|value| value.as_object())
        .expect("details present");
    assert_eq!(
        details.get("code").and_then(Value::as_str),
        Some(expected_code)
    );
    assert_eq!(
        details.get("field").and_then(Value::as_str),
        Some("interestThemeIds")
    );
    assert_eq!(details.get("index").and_then(Value::as_u64), Some(0));
}

#[test]
fn interests_request_validation_rejects_too_many_ids() {
    let payload = InterestsRequest {
        interest_theme_ids: vec![
            "3fa85f64-5717-4562-b3fc-2c963f66afa6".to_owned();
            INTEREST_THEME_IDS_MAX + 1
        ],
    };

    let err = parse_interest_theme_ids(payload).expect_err("too many ids");
    let api_error = map_interests_request_error(err);

    assert_eq!(api_error.code(), ErrorCode::InvalidRequest);
    assert_eq!(
        api_error.message(),
        "interest theme ids must contain at most 100 items"
    );
    let details = api_error
        .details()
        .and_then(|value| value.as_object())
        .expect("details present");
    assert_eq!(
        details.get("code").and_then(Value::as_str),
        Some("too_many_interest_theme_ids")
    );
    assert_eq!(
        details.get("field").and_then(Value::as_str),
        Some("interestThemeIds")
    );
    assert_eq!(
        details.get("max").and_then(Value::as_u64),
        Some(INTEREST_THEME_IDS_MAX as u64)
    );
    assert_eq!(
        details.get("count").and_then(Value::as_u64),
        Some((INTEREST_THEME_IDS_MAX + 1) as u64)
    );
}

#[test]
fn interests_request_validation_accepts_valid_ids() {
    let payload = InterestsRequest {
        interest_theme_ids: vec!["3fa85f64-5717-4562-b3fc-2c963f66afa6".to_owned()],
    };

    let parsed = parse_interest_theme_ids(payload).expect("valid interest theme ids");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].as_ref(), "3fa85f64-5717-4562-b3fc-2c963f66afa6");
}
