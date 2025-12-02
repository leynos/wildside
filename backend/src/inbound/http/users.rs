//! Users API handlers.
//!
//! ```text
//! POST /api/v1/login {"username":"admin","password":"password"}
//! GET /api/v1/users
//! ```

use crate::domain::{DisplayName, Error, LoginCredentials, LoginValidationError, User, UserId};
use crate::inbound::http::auth::authenticate;
use crate::inbound::http::session::SessionContext;
use crate::inbound::http::ApiResult;
use actix_web::{get, post, web, HttpResponse};
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
        (status = 400, description = "Invalid request", body = Error),
        (status = 401, description = "Invalid credentials", body = Error),
        (status = 500, description = "Internal server error")
    ),
    tags = ["users"],
    operation_id = "login",
    security([])
)]
#[post("/login")]
pub async fn login(
    session: SessionContext,
    payload: web::Json<LoginRequest>,
) -> ApiResult<HttpResponse> {
    let credentials =
        LoginCredentials::try_from(payload.into_inner()).map_err(map_login_validation_error)?;
    let user_id = authenticate(&credentials)?;
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
        (status = 200, description = "Users", body = [User]),
        (status = 400, description = "Invalid request", body = Error),
        (status = 401, description = "Unauthorised", body = Error),
        (status = 403, description = "Forbidden", body = Error),
        (status = 404, description = "Not found", body = Error),
        (status = 500, description = "Internal server error", body = Error)
    ),
    tags = ["users"],
    operation_id = "listUsers"
)]
#[get("/users")]
pub async fn list_users(session: SessionContext) -> ApiResult<web::Json<Vec<User>>> {
    session.require_user_id()?;
    const FIXTURE_ID: &str = "3fa85f64-5717-4562-b3fc-2c963f66afa6";
    const FIXTURE_DISPLAY_NAME: &str = "Ada Lovelace";

    // These values are compile-time constants; surface invalid data as an
    // internal error so automated checks catch accidental regressions.
    let id = UserId::new(FIXTURE_ID)
        .map_err(|err| Error::internal(format!("invalid fixture user id: {err}")))?;
    let display_name = DisplayName::new(FIXTURE_DISPLAY_NAME)
        .map_err(|err| Error::internal(format!("invalid fixture display name: {err}")))?;
    let data = vec![User::new(id, display_name)];
    Ok(web::Json(data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test as actix_test, web, App};
    use rstest::rstest;
    use serde_json::Value;

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
        App::new()
            .wrap(crate::inbound::http::test_utils::test_session_middleware())
            .service(web::scope("/api/v1").service(login).service(list_users))
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
}
