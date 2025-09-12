//! Users API handlers.
//!
//! ```text
//! POST /api/v1/login {"username":"admin","password":"password"}
//! GET /api/v1/users
//! ```

use crate::models::{ApiResult, Error, ErrorCode, User};
use actix_session::Session;
use actix_web::{get, post, web, HttpResponse, Result};
use serde::Deserialize;

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
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
    operation_id = "login"
)]
#[post("/login")]
pub async fn login(session: Session, payload: web::Json<LoginRequest>) -> Result<HttpResponse> {
    if payload.username == "admin" && payload.password == "password" {
        // In a real system, insert the authenticated user's ID.
        session.insert("user_id", "123e4567-e89b-12d3-a456-426614174000")?;
        Ok(HttpResponse::Ok().finish())
    } else {
        // Map to the shared Error type so ResponseError renders the JSON body.
        Err(Error {
            code: ErrorCode::Unauthorized,
            message: "invalid credentials".into(),
            trace_id: None,
            details: None,
        }
        .into())
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
    let data = vec![User {
        id: "3fa85f64-5717-4562-b3fc-2c963f66afa6".into(),
        display_name: "Ada Lovelace".into(),
    }];
    Ok(web::Json(data))
}
