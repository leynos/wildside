//! Users API handlers.
//!
//! ```text
//! POST /api/v1/login {"username":"admin","password":"password"}
//! GET /api/v1/users
//! ```

use crate::domain::{
    ApiResult, DisplayName, Error, LoginCredentials, LoginValidationError, User, UserId,
};
use actix_session::Session;
use actix_web::{get, post, web, HttpResponse, Result};
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
        (status = 401, description = "Invalid credentials", body = Error),
        (status = 500, description = "Internal server error")
    ),
    tags = ["users"],
    operation_id = "login",
    security([])
)]
#[post("/login")]
pub async fn login(session: Session, payload: web::Json<LoginRequest>) -> Result<HttpResponse> {
    let credentials =
        LoginCredentials::try_from(payload.into_inner()).map_err(map_login_validation_error)?;
    if credentials.username() == "admin" && credentials.password() == "password" {
        // In a real system, insert the authenticated user's ID.
        session.insert("user_id", "123e4567-e89b-12d3-a456-426614174000")?;
        Ok(HttpResponse::Ok().finish())
    } else {
        // Map to the shared Error type so ResponseError renders the JSON body.
        Err(Error::unauthorized("invalid credentials").into())
    }
}

fn map_login_validation_error(err: LoginValidationError) -> Error {
    match err {
        LoginValidationError::EmptyUsername => Error::invalid_request("username must not be empty")
            .with_details(json!({ "field": "username", "code": "empty_username" })),
        LoginValidationError::EmptyPassword => Error::invalid_request("password must not be empty")
            .with_details(json!({ "field": "password", "code": "empty_password" })),
    }
}
//

/// List known users.
///
/// # Examples
/// ```
/// use actix_web::App;
/// use backend::api::users::list_users;
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
pub async fn list_users(session: Session) -> ApiResult<web::Json<Vec<User>>> {
    if session
        .get::<String>("user_id")
        .map_err(actix_web::Error::from)?
        .is_none()
    {
        return Err(Error::unauthorized("login required"));
    }
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
    use actix_session::{storage::CookieSessionStore, SessionMiddleware};
    use actix_web::cookie::Key;
    use actix_web::{test, web, App};
    use serde_json::Value;

    #[actix_web::test]
    async fn login_rejects_invalid_payload() {
        let app = test::init_service(
            App::new()
                .wrap(
                    SessionMiddleware::builder(CookieSessionStore::default(), Key::generate())
                        .cookie_name("actix-session".to_owned())
                        .cookie_secure(false)
                        .build(),
                )
                .service(web::scope("/api/v1").service(login)),
        )
        .await;

        let request = test::TestRequest::post()
            .uri("/api/v1/login")
            .set_json(&LoginRequest {
                username: "   ".into(),
                password: "password".into(),
            })
            .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), actix_web::http::StatusCode::BAD_REQUEST);
        let body = test::read_body(response).await;
        let value: Value = serde_json::from_slice(&body).expect("error payload");
        assert_eq!(
            value.get("message").and_then(Value::as_str),
            Some("username must not be empty")
        );
    }

    #[actix_web::test]
    async fn login_rejects_empty_password() {
        let app = test::init_service(
            App::new()
                .wrap(
                    SessionMiddleware::builder(CookieSessionStore::default(), Key::generate())
                        .cookie_name("actix-session".to_owned())
                        .cookie_secure(false)
                        .build(),
                )
                .service(web::scope("/api/v1").service(login)),
        )
        .await;

        let request = test::TestRequest::post()
            .uri("/api/v1/login")
            .set_json(&LoginRequest {
                username: "admin".into(),
                password: String::new(),
            })
            .to_request();

        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), actix_web::http::StatusCode::BAD_REQUEST);
        let body = test::read_body(response).await;
        let value: Value = serde_json::from_slice(&body).expect("error payload");
        assert_eq!(
            value.get("message").and_then(Value::as_str),
            Some("password must not be empty")
        );
    }

    #[actix_web::test]
    async fn list_users_returns_camel_case_json() {
        let app = test::init_service(
            App::new()
                .wrap(
                    SessionMiddleware::builder(CookieSessionStore::default(), Key::generate())
                        .cookie_name("actix-session".to_owned())
                        .cookie_secure(false)
                        .build(),
                )
                .service(web::scope("/api/v1").service(login).service(list_users)),
        )
        .await;

        let login_req = test::TestRequest::post()
            .uri("/api/v1/login")
            .set_json(&LoginRequest {
                username: "admin".into(),
                password: "password".into(),
            })
            .to_request();
        let login_res = test::call_service(&app, login_req).await;
        assert!(login_res.status().is_success());
        let cookie = login_res
            .response()
            .cookies()
            .find(|c| c.name() == "actix-session")
            .expect("actix-session cookie");

        let users_req = test::TestRequest::get()
            .uri("/api/v1/users")
            .cookie(cookie)
            .to_request();
        let users_res = test::call_service(&app, users_req).await;
        assert!(users_res.status().is_success());
        let body = test::read_body(users_res).await;
        let value: Value = serde_json::from_slice(&body).expect("response JSON");
        let first = &value.as_array().expect("array")[0];
        assert_eq!(
            first.get("displayName").and_then(Value::as_str),
            Some("Ada Lovelace")
        );
        assert!(first.get("display_name").is_none());
    }
}
