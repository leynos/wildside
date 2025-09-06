//! Users API handlers.

use crate::models::User;
use actix_session::Session;
use actix_web::{get, http::StatusCode, post, web, HttpResponse, ResponseError, Result};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, utoipa::ToSchema)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ErrorResponse {
    pub code: &'static str,
    pub message: &'static str,
}

impl ErrorResponse {
    pub fn unauthorized(message: &'static str) -> Self {
        Self {
            code: "unauthorized",
            message,
        }
    }
}

impl std::fmt::Display for ErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl ResponseError for ErrorResponse {
    fn status_code(&self) -> StatusCode {
        StatusCode::UNAUTHORIZED
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::Unauthorized().json(self)
    }
}

/// Authenticate user and establish a session.
#[utoipa::path(
    post,
    path = "/api/v1/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login success", headers(("Set-Cookie" = String, description = "Session cookie"))),
        (status = 401, description = "Invalid credentials", body = ErrorResponse),
        (status = 500, description = "Internal server error")
    ),
    tags = ["users"],
    operation_id = "login"
)]
#[post("/login")]
pub async fn login(session: Session, payload: web::Json<LoginRequest>) -> Result<HttpResponse> {
    if payload.username == "admin" && payload.password == "password" {
        session.insert("user_id", "123e4567-e89b-12d3-a456-426614174000")?;
        Ok(HttpResponse::Ok().finish())
    } else {
        Err(ErrorResponse::unauthorized("invalid credentials").into())
    }
}

/// List known users.
#[utoipa::path(
    get,
    path = "/api/v1/users",
    responses(
        (status = 200, description = "Users", body = [User]),
        (status = 401, description = "Unauthorised", body = ErrorResponse),
        (status = 500, description = "Internal server error")
    ),
    tags = ["users"],
    operation_id = "listUsers"
)]
#[get("/users")]
pub async fn list_users(session: Session) -> Result<web::Json<Vec<User>>> {
    if session.get::<String>("user_id")?.is_none() {
        return Err(ErrorResponse::unauthorized("login required").into());
    }

    let data = vec![User {
        id: "3fa85f64-5717-4562-b3fc-2c963f66afa6".into(),
        display_name: "Ada Lovelace".into(),
    }];
    Ok(web::Json(data))
}
