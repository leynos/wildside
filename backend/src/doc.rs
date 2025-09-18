//! OpenAPI documentation setup.

use crate::models::{Error, ErrorCode, User};
use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa::{Modify, OpenApi};

/// Enrich the generated document with the session cookie security scheme.
struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi
            .components
            .get_or_insert_with(utoipa::openapi::Components::default);

        components.add_security_scheme(
            "SessionCookie",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::with_description(
                "session",
                "Session cookie issued by POST /api/v1/login.",
            ))),
        );
    }
}

/// OpenAPI document for the REST API.
/// Swagger UI is enabled in debug builds only and used by tooling.
#[derive(OpenApi)]
#[openapi(
    modifiers(&SecurityAddon),
    info(
        title = "Wildside backend API",
        description = "HTTP interface for session-authenticated access and health probes.",
        license(
            name = "Apache-2.0",
            url = "https://www.apache.org/licenses/LICENSE-2.0.html"
        )
    ),
    servers(
        (url = "/", description = "Relative to the deployment base URL")
    ),
    security(("SessionCookie" = [])),
    paths(
        crate::api::users::list_users,
        crate::api::users::login,
        crate::api::health::ready,
        crate::api::health::live,
    ),
    components(schemas(User, Error, ErrorCode)),
    tags(
        (name = "users", description = "Operations related to users"),
        (name = "health", description = "Endpoints for health checks")
    )
)]
pub struct ApiDoc;
