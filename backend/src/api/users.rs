//! Users API handlers.

use crate::models::{Error, ErrorCode, User};
use actix_web::{get, web};
use serde::Deserialize;

#[derive(Deserialize)]
struct ListUsersParams {
    /// When set, forces an error response so clients can exercise failure handling.
    fail: Option<String>,
}

/// List known users.
#[utoipa::path(
    get,
    path = "/api/users",
    responses(
        (status = 200, description = "Users", body = [User]),
        (status = 400, description = "Bad request", body = Error),
        (status = 401, description = "Unauthorised", body = Error),
        (status = 403, description = "Forbidden", body = Error),
        (status = 404, description = "Not found", body = Error),
        (status = 500, description = "Internal server error", body = Error)
    ),
    tags = ["Users"],
    operation_id = "listUsers",
)]
#[get("/api/users")]
pub async fn list_users(
    params: web::Query<ListUsersParams>,
) -> Result<web::Json<Vec<User>>, Error> {
    if let Some(kind) = params.fail.as_deref() {
        // Fail early to exercise error handling in tests.
        let err = match kind {
            "unauthorized" => Error {
                code: ErrorCode::Unauthorized,
                message: "unauthorised".into(),
                trace_id: None,
                details: None,
            },
            "forbidden" => Error {
                code: ErrorCode::Forbidden,
                message: "forbidden".into(),
                trace_id: None,
                details: None,
            },
            "invalid" => Error {
                code: ErrorCode::InvalidRequest,
                message: "invalid request".into(),
                trace_id: None,
                details: None,
            },
            _ => Error {
                code: ErrorCode::InternalError,
                message: "unexpected".into(),
                trace_id: None,
                details: None,
            },
        };
        return Err(err);
    }

    let data = vec![User {
        id: "u_1".into(),
        display_name: "Ada".into(),
    }];
    Ok(web::Json(data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{http::StatusCode, test, App};

    #[actix_web::test]
    async fn returns_unauthorized() {
        // Simulate missing credentials to ensure mapping to 401.
        let app = test::init_service(App::new().service(list_users)).await;
        let req = test::TestRequest::get()
            .uri("/api/users?fail=unauthorized")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let body: Error = test::read_body_json(resp).await;
        assert!(matches!(body.code, ErrorCode::Unauthorized));
    }

    #[actix_web::test]
    async fn maps_invalid_request() {
        // Invalid parameter must surface as 400 to guide callers.
        let app = test::init_service(App::new().service(list_users)).await;
        let req = test::TestRequest::get()
            .uri("/api/users?fail=invalid")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body: Error = test::read_body_json(resp).await;
        assert!(matches!(body.code, ErrorCode::InvalidRequest));
    }
}
