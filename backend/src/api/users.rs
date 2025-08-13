//! Users API handlers.

use crate::models::user::User;
use actix_web::{get, HttpResponse};

#[utoipa::path(
    get,
    path = "/api/users",
    responses(
        (status = 200, description = "Users", body = [User]),
        (status = 401, description = "Unauthorised"),
        (status = 500, description = "Internal server error")
    )
)]
#[get("/api/users")]
pub async fn list_users() -> HttpResponse {
    let data = vec![User {
        id: "u_1".into(),
        display_name: "Ada".into(),
    }];
    HttpResponse::Ok().json(data)
}
