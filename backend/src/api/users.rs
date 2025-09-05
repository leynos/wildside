//! Users API handlers.

use crate::models::User;
use actix_web::{get, web, Result};
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
    tags = ["Users"],
    operation_id = "listUsers"
)]
#[get("/api/users")]
pub async fn list_users() -> Result<web::Json<Vec<User>>> {
    let data = vec![User {
        id: Uuid::nil(),
        display_name: "Ada".into(),
    }];
    Ok(web::Json(data))
}
