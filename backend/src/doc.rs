//! OpenAPI documentation setup.

use crate::models::{Error, ErrorCode, User};
use utoipa::{openapi, Modify, OpenApi};

/// OpenAPI document for the REST API.
/// Swagger UI is enabled in debug builds only and used by tooling.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Wildside API",
        description = "Public REST API for Wildside services.",
        license(
            name = "ISC",
            url = "https://opensource.org/license/isc-license-txt/"
        )
    ),
    servers(
        (url = "https://api.wildside.test", description = "Local development server")
    ),
    paths(
        crate::api::users::list_users,
        crate::api::users::login,
        crate::api::health::ready,
        crate::api::health::live,
    ),
    components(schemas(User, Error, ErrorCode)),
    modifiers(&CookieSecurity),
    security(("cookieAuth" = [])),
    tags(
        (name = "users", description = "Operations related to users"),
        (name = "health", description = "Endpoints for health checks")
    )
)]
pub struct ApiDoc;

/// Adds the cookie-based session scheme so generated docs reflect runtime auth.
struct CookieSecurity;

impl Modify for CookieSecurity {
    fn modify(&self, openapi: &mut openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);

        components.add_security_scheme(
            "cookieAuth",
            openapi::security::SecurityScheme::ApiKey(openapi::security::ApiKey::Cookie(
                openapi::security::ApiKeyValue::new("session"),
            )),
        );
    }
}
