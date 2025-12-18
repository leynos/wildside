//! OpenAPI documentation setup.

use crate::inbound::http::schemas::{ErrorCodeSchema, ErrorSchema, UserSchema};
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
        crate::inbound::http::users::list_users,
        crate::inbound::http::users::login,
        crate::inbound::http::health::ready,
        crate::inbound::http::health::live,
    ),
    components(schemas(UserSchema, ErrorSchema, ErrorCodeSchema)),
    tags(
        (name = "users", description = "Operations related to users"),
        (name = "health", description = "Endpoints for health checks")
    )
)]
pub struct ApiDoc;

#[cfg(test)]
mod tests {
    use super::*;
    use utoipa::OpenApi;

    // Note: utoipa replaces :: with . in schema names
    const ERROR_SCHEMA_NAME: &str = "crate.domain.Error";
    const ERROR_CODE_SCHEMA_NAME: &str = "crate.domain.ErrorCode";
    const USER_SCHEMA_NAME: &str = "crate.domain.User";

    #[test]
    fn openapi_document_contains_schema_wrappers() {
        let doc = ApiDoc::openapi();
        let json = doc.to_json().expect("valid JSON");

        // Verify schemas are registered under domain type names (via #[schema(as = ...)])
        assert!(
            json.contains(&format!("\"{ERROR_SCHEMA_NAME}\"")),
            "Error schema should be registered"
        );
        assert!(
            json.contains(&format!("\"{ERROR_CODE_SCHEMA_NAME}\"")),
            "ErrorCode schema should be registered"
        );
        assert!(
            json.contains(&format!("\"{USER_SCHEMA_NAME}\"")),
            "User schema should be registered"
        );
    }

    #[test]
    fn openapi_document_references_schema_types_in_responses() {
        let doc = ApiDoc::openapi();
        let json = doc.to_json().expect("valid JSON");

        // Verify responses reference the schema types
        // The paths should reference Error and User in their response schemas
        assert!(
            json.contains(&format!("#/components/schemas/{ERROR_SCHEMA_NAME}")),
            "Responses should reference Error schema"
        );
        assert!(
            json.contains(&format!("#/components/schemas/{USER_SCHEMA_NAME}")),
            "Responses should reference User schema"
        );
    }

    #[test]
    fn openapi_error_schema_has_required_fields() {
        let doc = ApiDoc::openapi();
        let components = doc.components.as_ref().expect("components present");
        let schemas = &components.schemas;

        // Check that the Error schema exists and has the expected structure
        let error_schema = schemas
            .get(ERROR_SCHEMA_NAME)
            .expect("Error schema registered");
        let schema_str = serde_json::to_string(error_schema).expect("serialise");

        assert!(
            schema_str.contains("\"code\""),
            "Error schema should have code field"
        );
        assert!(
            schema_str.contains("\"message\""),
            "Error schema should have message field"
        );
    }

    #[test]
    fn openapi_user_schema_has_required_fields() {
        let doc = ApiDoc::openapi();
        let components = doc.components.as_ref().expect("components present");
        let schemas = &components.schemas;

        // Check that the User schema exists and has the expected structure
        let user_schema = schemas
            .get(USER_SCHEMA_NAME)
            .expect("User schema registered");
        let schema_str = serde_json::to_string(user_schema).expect("serialise");

        assert!(
            schema_str.contains("\"id\""),
            "User schema should have id field"
        );
        assert!(
            schema_str.contains("\"display_name\""),
            "User schema should have display_name field"
        );
    }
}
