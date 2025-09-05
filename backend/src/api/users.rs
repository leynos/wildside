//! Users API handlers.

use crate::models::User;
use actix_session::Session;
use actix_web::{error::ErrorUnauthorized, get, web, Result};
use uuid::Uuid;

/// List known users.
#[utoipa::path(
    get,
    path = "/api/users",
    responses(
        (status = 200, description = "Users", body = [User]),
        (status = 401, description = "Unauthorised"),
        (status = 500, description = "Internal server error")
    ),
    tags = ["users"],
    operation_id = "listUsers"
)]
#[get("/api/users")]
pub async fn list_users(session: Session) -> Result<web::Json<Vec<User>>> {
    if session.get::<String>("user_id")?.is_none() {
        return Err(ErrorUnauthorized("unauthorised"));
    }

    let data = vec![User {
        id: Uuid::new_v4(),
        display_name: "Ada".into(),
    }];
    Ok(web::Json(data))
}
