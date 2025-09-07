//! OpenAPI documentation setup.

use crate::models::{Error, ErrorCode, User};
use utoipa::OpenApi;

/// OpenAPI document for the REST API.
/// Swagger UI is enabled in debug builds only and used by tooling.
#[derive(OpenApi)]
#[openapi(
<<<<<<< HEAD
    paths(
        crate::api::users::list_users,
        crate::api::users::login,
        crate::api::health::ready,
        crate::api::health::live,
    ),
    components(schemas(User, Error, ErrorCode)),
    tags((name = "users", description = "Operations related to users"))
||||||| parent of 526744b (Tag health endpoints in OpenAPI spec)
    paths(crate::api::users::list_users, crate::api::health::ready, crate::api::health::live),
    components(schemas(User)),
    tags((name = "users", description = "Operations related to users"))
=======
    paths(crate::api::users::list_users, crate::api::health::ready, crate::api::health::live),
    components(schemas(User)),
    tags(
        (name = "users", description = "Operations related to users"),
        (name = "health", description = "Endpoints for health checks")
    )
>>>>>>> 526744b (Tag health endpoints in OpenAPI spec)
)]
pub struct ApiDoc;
