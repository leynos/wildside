//! Users API handlers.

use crate::models::{Error, User};
use actix_web::{get, web};

/// List known users.
#[utoipa::path(
    get,
    path = "/api/users",
    responses(
        (status = 200, description = "Users", body = [User]),
        (status = 401, description = "Unauthorised", body = Error),
        (status = 403, description = "Forbidden", body = Error),
        (status = 500, description = "Internal server error", body = Error)
    ),
    tags = ["Users"],
    operation_id = "listUsers",
)]
#[get("/api/users")]
pub async fn list_users() -> Result<web::Json<Vec<User>>, Error> {
    let data = vec![User {
        id: "u_1".into(),
        display_name: "Ada".into(),
    }];
    Ok(web::Json(data))
}
