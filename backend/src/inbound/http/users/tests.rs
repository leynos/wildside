//! Tests for users API handlers.

use super::*;
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
use std::{error::Error as StdError, io, sync::Arc};

type TestResult<T = ()> = Result<T, Box<dyn StdError>>;

mod request_validation_tests;

#[derive(Debug)]
struct ValidationExpectation<'a> {
    message: &'a str,
    field: &'a str,
    code: &'a str,
    top_code: &'a str,
}

fn get_details_object(value: &Value) -> io::Result<&serde_json::Map<String, Value>> {
    value
        .get("details")
        .and_then(Value::as_object)
        .ok_or_else(|| io::Error::other("expected details object to be present"))
}

async fn assert_login_validation_error(
    username: &str,
    password: &str,
    expected: ValidationExpectation<'_>,
) -> TestResult {
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
    let value: Value = serde_json::from_slice(&body)?;
    assert_eq!(
        value.get("message").and_then(Value::as_str),
        Some(expected.message)
    );
    assert_eq!(
        value.get("code").and_then(Value::as_str),
        Some(expected.top_code)
    );
    let details = get_details_object(&value)?;
    assert_eq!(
        details.get("field").and_then(Value::as_str),
        Some(expected.field)
    );
    assert_eq!(
        details.get("code").and_then(Value::as_str),
        Some(expected.code)
    );
    Ok(())
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
        .ok_or_else(|| io::Error::other("missing session cookie in login response"))?
        .into_owned())
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
) -> TestResult {
    assert_login_validation_error(username, password, expected).await
}

#[actix_web::test]
async fn login_rejects_wrong_credentials_with_unauthorized_status() -> TestResult {
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
    let value: Value = serde_json::from_slice(&body)?;
    assert_eq!(
        value.get("message").and_then(Value::as_str),
        Some("invalid credentials")
    );
    assert_eq!(
        value.get("code").and_then(Value::as_str),
        Some("unauthorized")
    );
    Ok(())
}

#[actix_web::test]
async fn list_users_returns_camel_case_json() -> TestResult {
    let app = actix_test::init_service(test_app()).await;
    let cookie = login_and_get_cookie(&app).await?;

    let users_req = actix_test::TestRequest::get()
        .uri("/api/v1/users")
        .cookie(cookie)
        .to_request();
    let users_res = actix_test::call_service(&app, users_req).await;
    assert!(users_res.status().is_success());
    let body = actix_test::read_body(users_res).await;
    let value: Value = serde_json::from_slice(&body)?;
    assert_eq!(value.get("limit").and_then(Value::as_u64), Some(20));
    let links = value
        .get("links")
        .and_then(Value::as_object)
        .ok_or_else(|| io::Error::other("expected links object"))?;
    assert!(
        links
            .get("self")
            .and_then(Value::as_str)
            .is_some_and(|link| link.ends_with("/api/v1/users?limit=20"))
    );
    assert!(links.get("next").is_none());
    assert!(links.get("prev").is_none());

    let first = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| io::Error::other("expected users response data array"))?
        .first()
        .ok_or_else(|| io::Error::other("expected at least one user in response"))?;
    assert_eq!(
        first.get("displayName").and_then(Value::as_str),
        Some("Ada Lovelace")
    );
    assert!(first.get("createdAt").is_some());
    assert!(first.get("display_name").is_none());
    Ok(())
}

#[rstest]
#[case("/api/v1/users?limit=0")]
#[case("/api/v1/users?limit=200")]
#[case("/api/v1/users?limit=not-a-number")]
#[actix_web::test]
async fn list_users_rejects_invalid_limits(#[case] path: &str) -> TestResult {
    let app = actix_test::init_service(test_app()).await;
    let cookie = login_and_get_cookie(&app).await?;

    let users_req = actix_test::TestRequest::get()
        .uri(path)
        .cookie(cookie)
        .to_request();
    let users_res = actix_test::call_service(&app, users_req).await;

    assert_eq!(users_res.status(), actix_web::http::StatusCode::BAD_REQUEST);
    let body = actix_test::read_body(users_res).await;
    let value: Value = serde_json::from_slice(&body)?;
    assert_eq!(
        value.get("message").and_then(Value::as_str),
        Some("limit must be between 1 and 100")
    );
    let details = get_details_object(&value)?;
    assert_eq!(details.get("field").and_then(Value::as_str), Some("limit"));
    assert_eq!(
        details.get("code").and_then(Value::as_str),
        Some("invalid_limit")
    );
    Ok(())
}

#[actix_web::test]
async fn list_users_rejects_invalid_cursor() -> TestResult {
    let app = actix_test::init_service(test_app()).await;
    let cookie = login_and_get_cookie(&app).await?;

    let users_req = actix_test::TestRequest::get()
        .uri("/api/v1/users?cursor=not-a-cursor")
        .cookie(cookie)
        .to_request();
    let users_res = actix_test::call_service(&app, users_req).await;

    assert_eq!(users_res.status(), actix_web::http::StatusCode::BAD_REQUEST);
    let body = actix_test::read_body(users_res).await;
    let value: Value = serde_json::from_slice(&body)?;
    assert_eq!(
        value.get("message").and_then(Value::as_str),
        Some("cursor is invalid")
    );
    let details = get_details_object(&value)?;
    assert_eq!(details.get("field").and_then(Value::as_str), Some("cursor"));
    assert_eq!(
        details.get("code").and_then(Value::as_str),
        Some("invalid_cursor")
    );
    Ok(())
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
async fn update_interests_rejects_too_many_ids() -> TestResult {
    let app = actix_test::init_service(test_app()).await;
    let cookie = login_and_get_cookie(&app).await?;
    let payload = InterestsRequest {
        interest_theme_ids: vec![
            "3fa85f64-5717-4562-b3fc-2c963f66afa6".to_owned();
            INTEREST_THEME_IDS_MAX + 1
        ],
        expected_revision: None,
    };

    let request = actix_test::TestRequest::put()
        .uri("/api/v1/users/me/interests")
        .cookie(cookie)
        .set_json(payload)
        .to_request();
    let response = actix_test::call_service(&app, request).await;

    assert_eq!(response.status(), actix_web::http::StatusCode::BAD_REQUEST);
    let body = actix_test::read_body(response).await;
    let value: Value = serde_json::from_slice(&body)?;
    assert_eq!(
        value.get("message").and_then(Value::as_str),
        Some("interest theme ids must contain at most 100 items")
    );
    assert_eq!(
        value.get("code").and_then(Value::as_str),
        Some("invalid_request")
    );
    let details = get_details_object(&value)?;
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
    Ok(())
}

#[test]
fn interests_request_serializes_expected_revision_in_camel_case() -> TestResult {
    let request = InterestsRequest {
        interest_theme_ids: vec![],
        expected_revision: Some(3),
    };

    let value = serde_json::to_value(request)?;
    assert_eq!(
        value.get("expectedRevision").and_then(Value::as_u64),
        Some(3)
    );
    assert!(
        value.get("expected_revision").is_none(),
        "snake_case key should not be present"
    );
    Ok(())
}
