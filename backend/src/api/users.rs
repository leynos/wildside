//! Users API handlers.

use crate::models::User;
use actix_session::Session;
use actix_web::{error::ErrorUnauthorized, get, post, web, HttpResponse, Result};
use serde::Deserialize;

#[derive(Deserialize, utoipa::ToSchema)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// Authenticate user and establish a session.
#[utoipa::path(
    post,
    path = "/api/v1/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login success", headers(("Set-Cookie" = String, description = "Session cookie"))),
        (status = 401, description = "Invalid credentials"),
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
        Err(ErrorUnauthorized("invalid credentials"))
    }
}

/// List known users.
#[utoipa::path(
    get,
    path = "/api/v1/users",
    responses(
        (status = 200, description = "Users", body = [User]),
        (status = 401, description = "Unauthorised"),
        (status = 500, description = "Internal server error")
    ),
    tags = ["users"],
    operation_id = "listUsers"
)]
#[get("/users")]
pub async fn list_users(session: Session) -> Result<web::Json<Vec<User>>> {
    if session.get::<String>("user_id")?.is_none() {
        return Err(ErrorUnauthorized("unauthorised"));
    }

    let data = vec![User {
        id: "3fa85f64-5717-4562-b3fc-2c963f66afa6".into(),
        display_name: "Ada Lovelace".into(),
    }];
    Ok(web::Json(data))
}
