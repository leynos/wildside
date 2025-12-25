//! Users API handlers.
//!
//! ```text
//! POST /api/v1/login {"username":"admin","password":"password"}
//! GET /api/v1/users
//! GET /api/v1/users/me
//! PUT /api/v1/users/me/interests
//! ```

use crate::domain::{
    Error, InterestThemeId, InterestThemeIdValidationError, LoginCredentials, LoginValidationError,
    User, UserInterests,
};
use crate::inbound::http::schemas::{ErrorSchema, UserInterestsSchema, UserSchema};
use crate::inbound::http::session::SessionContext;
use crate::inbound::http::state::HttpState;
use crate::inbound::http::ApiResult;
use actix_web::{get, post, put, web, HttpResponse};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Login request body for `POST /api/v1/login`.
///
/// Example JSON:
/// `{"username":"admin","password":"password"}`
#[derive(Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// Interest theme update payload for `PUT /api/v1/users/me/interests`.
#[derive(Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InterestsRequest {
    pub interest_theme_ids: Vec<String>,
}

#[derive(Debug)]
enum InterestsRequestError {
    InvalidInterestThemeId {
        index: usize,
        value: String,
        error: InterestThemeIdValidationError,
    },
}

fn parse_interest_theme_ids(
    payload: InterestsRequest,
) -> Result<Vec<InterestThemeId>, InterestsRequestError> {
    payload
        .interest_theme_ids
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            InterestThemeId::new(&value).map_err(|error| {
                InterestsRequestError::InvalidInterestThemeId {
                    index,
                    value,
                    error,
                }
            })
        })
        .collect()
}

impl TryFrom<LoginRequest> for LoginCredentials {
    type Error = LoginValidationError;

    fn try_from(value: LoginRequest) -> Result<Self, Self::Error> {
        Self::try_from_parts(&value.username, &value.password)
    }
}

/// Authenticate user and establish a session.
///
/// Uses the centralised `Error` type so clients get a consistent
/// error schema across all endpoints.
#[utoipa::path(
    post,
    path = "/api/v1/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login success", headers(("Set-Cookie" = String, description = "Session cookie"))),
        (status = 400, description = "Invalid request", body = ErrorSchema),
        (status = 401, description = "Invalid credentials", body = ErrorSchema),
        (status = 500, description = "Internal server error")
    ),
    tags = ["users"],
    operation_id = "login",
    security([])
)]
#[post("/login")]
pub async fn login(
    state: web::Data<HttpState>,
    session: SessionContext,
    payload: web::Json<LoginRequest>,
) -> ApiResult<HttpResponse> {
    let credentials =
        LoginCredentials::try_from(payload.into_inner()).map_err(map_login_validation_error)?;
    let user_id = state.login.authenticate(&credentials).await?;
    session.persist_user(&user_id)?;
    Ok(HttpResponse::Ok().finish())
}

fn map_login_validation_error(err: LoginValidationError) -> Error {
    match err {
        LoginValidationError::EmptyUsername => Error::invalid_request("username must not be empty")
            .with_details(json!({ "field": "username", "code": "empty_username" })),
        LoginValidationError::EmptyPassword => Error::invalid_request("password must not be empty")
            .with_details(json!({ "field": "password", "code": "empty_password" })),
    }
}

fn map_interests_request_error(err: InterestsRequestError) -> Error {
    match err {
        InterestsRequestError::InvalidInterestThemeId {
            index,
            value,
            error,
        } => {
            let (message, code) = match error {
                InterestThemeIdValidationError::EmptyId => (
                    "interest theme id must not be empty",
                    "empty_interest_theme_id",
                ),
                InterestThemeIdValidationError::InvalidId => (
                    "interest theme id must be a valid UUID",
                    "invalid_interest_theme_id",
                ),
            };
            Error::invalid_request(message).with_details(json!({
                "field": "interestThemeIds",
                "index": index,
                "value": value,
                "code": code,
            }))
        }
    }
}

/// List known users.
///
/// # Examples
/// ```
/// use actix_web::App;
/// use backend::inbound::http::users::list_users;
///
/// let app = App::new().service(list_users);
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/users",
    responses(
        (status = 200, description = "Users", body = [UserSchema]),
        (status = 400, description = "Invalid request", body = ErrorSchema),
        (status = 401, description = "Unauthorised", body = ErrorSchema),
        (status = 403, description = "Forbidden", body = ErrorSchema),
        (status = 404, description = "Not found", body = ErrorSchema),
        (status = 500, description = "Internal server error", body = ErrorSchema)
    ),
    tags = ["users"],
    operation_id = "listUsers"
)]
#[get("/users")]
pub async fn list_users(
    state: web::Data<HttpState>,
    session: SessionContext,
) -> ApiResult<web::Json<Vec<User>>> {
    let user_id = session.require_user_id()?;
    let data = state.users.list_users(&user_id).await?;
    Ok(web::Json(data))
}

/// Fetch the authenticated user's profile.
#[utoipa::path(
    get,
    path = "/api/v1/users/me",
    responses(
        (status = 200, description = "User profile", body = UserSchema),
        (status = 401, description = "Unauthorised", body = ErrorSchema),
        (status = 500, description = "Internal server error", body = ErrorSchema)
    ),
    tags = ["users"],
    operation_id = "currentUser"
)]
#[get("/users/me")]
pub async fn current_user(
    state: web::Data<HttpState>,
    session: SessionContext,
) -> ApiResult<web::Json<User>> {
    let user_id = session.require_user_id()?;
    let user = state.profile.fetch_profile(&user_id).await?;
    Ok(web::Json(user))
}

/// Update the authenticated user's interest theme selections.
#[utoipa::path(
    put,
    path = "/api/v1/users/me/interests",
    request_body = InterestsRequest,
    responses(
        (status = 200, description = "Updated interests", body = UserInterestsSchema),
        (status = 400, description = "Invalid request", body = ErrorSchema),
        (status = 401, description = "Unauthorised", body = ErrorSchema),
        (status = 500, description = "Internal server error", body = ErrorSchema)
    ),
    tags = ["users"],
    operation_id = "updateUserInterests"
)]
#[put("/users/me/interests")]
pub async fn update_interests(
    state: web::Data<HttpState>,
    session: SessionContext,
    payload: web::Json<InterestsRequest>,
) -> ApiResult<web::Json<UserInterests>> {
    let user_id = session.require_user_id()?;
    let interest_theme_ids =
        parse_interest_theme_ids(payload.into_inner()).map_err(map_interests_request_error)?;
    let interests = state
        .interests
        .set_interests(&user_id, interest_theme_ids)
        .await?;
    Ok(web::Json(interests))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ports::{
        FixtureLoginService, FixtureRouteSubmissionService, FixtureUserInterestsCommand,
        FixtureUserProfileQuery, FixtureUsersQuery,
    };
    use crate::domain::ErrorCode;
    use crate::inbound::http::state::HttpStatePorts;
    use actix_web::{test as actix_test, web, App};
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
            route_submission: Arc::new(FixtureRouteSubmissionService),
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

        let login_req = actix_test::TestRequest::post()
            .uri("/api/v1/login")
            .set_json(&LoginRequest {
                username: "admin".into(),
                password: "password".into(),
            })
            .to_request();
        let login_res = actix_test::call_service(&app, login_req).await;
        assert!(login_res.status().is_success());
        let cookie = login_res
            .response()
            .cookies()
            .find(|c| c.name() == "session")
            .expect("session cookie");

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
    fn interests_request_validation_accepts_valid_ids() {
        let payload = InterestsRequest {
            interest_theme_ids: vec!["3fa85f64-5717-4562-b3fc-2c963f66afa6".to_owned()],
        };

        let parsed = parse_interest_theme_ids(payload).expect("valid interest theme ids");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].as_ref(), "3fa85f64-5717-4562-b3fc-2c963f66afa6");
    }
}
