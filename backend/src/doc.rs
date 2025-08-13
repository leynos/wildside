//! OpenAPI documentation setup.

use crate::api::users::__path_list_users;
use crate::models::user::User;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(paths(list_users), components(schemas(User)), tags((name = "users")))]
pub struct ApiDoc;
