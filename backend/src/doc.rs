//! OpenAPI documentation setup.

use crate::models::user::User;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(crate::api::users::list_users),
    components(schemas(User)),
    tags((name = "users"))
)]
pub struct ApiDoc;
