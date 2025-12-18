//! OpenAPI documentation configuration.
//!
//! This module defines the [`ApiDoc`] struct which generates the OpenAPI
//! specification for the REST API. It registers:
//!
//! - **Paths**: All HTTP endpoints from the inbound layer (users, health)
//! - **Schemas**: Domain type wrappers ([`ErrorSchema`], [`ErrorCodeSchema`],
//!   [`UserSchema`]) that provide OpenAPI definitions without coupling domain
//!   types to the utoipa framework
//! - **Security**: Session cookie authentication scheme
//!
//! The generated specification is used by Swagger UI (debug builds) and
//! exported via `cargo run --bin openapi-dump` for external tooling.

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
    //! Tests verifying OpenAPI schema field structure.
    //!
    //! Schema registration and endpoint reference tests are covered by the
    //! BDD tests in `backend/tests/openapi_schemas_bdd.rs`.

    use super::*;
    use utoipa::openapi::schema::Schema;
    use utoipa::openapi::RefOr;
    use utoipa::OpenApi;

    // Note: utoipa replaces :: with . in schema names
    const ERROR_SCHEMA_NAME: &str = "crate.domain.Error";
    const USER_SCHEMA_NAME: &str = "crate.domain.User";

    /// Assert that an Object schema contains a field with the given name.
    fn assert_object_schema_has_field(schema: &RefOr<Schema>, field: &str) {
        match schema {
            RefOr::T(Schema::Object(obj)) => {
                assert!(
                    obj.properties.contains_key(field),
                    "schema should have field '{field}'"
                );
            }
            _ => panic!("expected Object schema"),
        }
    }

    #[test]
    fn openapi_error_schema_has_required_fields() {
        let doc = ApiDoc::openapi();
        let schemas = &doc.components.as_ref().expect("components").schemas;
        let error_schema = schemas.get(ERROR_SCHEMA_NAME).expect("Error schema");

        assert_object_schema_has_field(error_schema, "code");
        assert_object_schema_has_field(error_schema, "message");
    }

    #[test]
    fn openapi_user_schema_has_required_fields() {
        let doc = ApiDoc::openapi();
        let schemas = &doc.components.as_ref().expect("components").schemas;
        let user_schema = schemas.get(USER_SCHEMA_NAME).expect("User schema");

        assert_object_schema_has_field(user_schema, "id");
        assert_object_schema_has_field(user_schema, "display_name");
    }
}
