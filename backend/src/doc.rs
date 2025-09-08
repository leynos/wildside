//! OpenAPI documentation setup.

use crate::models::{Error, ErrorCode, User};
use utoipa::OpenApi;

/// OpenAPI document for the REST API.
/// Swagger UI is enabled in debug builds only and used by tooling.
#[derive(OpenApi)]
#[openapi(
    paths(
        crate::api::users::list_users,
        crate::api::users::login,
    ),
    components(schemas(User, Error, ErrorCode)),
    tags((name = "users", description = "Operations related to users"))
)]
pub struct ApiDoc;
