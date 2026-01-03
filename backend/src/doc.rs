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

use crate::inbound::http::schemas::{
    ErrorCodeSchema, ErrorSchema, InterestThemeIdSchema, UserInterestsSchema, UserSchema,
};
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
        crate::inbound::http::users::current_user,
        crate::inbound::http::users::update_interests,
        crate::inbound::http::preferences::get_preferences,
        crate::inbound::http::preferences::update_preferences,
        crate::inbound::http::health::ready,
        crate::inbound::http::health::live,
        crate::inbound::http::annotations::get_annotations,
        crate::inbound::http::annotations::upsert_note,
        crate::inbound::http::annotations::update_progress,
    ),
    components(schemas(
        UserSchema,
        UserInterestsSchema,
        InterestThemeIdSchema,
        ErrorSchema,
        ErrorCodeSchema
    )),
    tags(
        (name = "users", description = "Operations related to users"),
        (name = "routes", description = "Operations related to routes"),
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
    use crate::test_support::openapi::unwrap_object_schema;
    use utoipa::OpenApi;
    use utoipa::openapi::RefOr;
    use utoipa::openapi::schema::Schema;

    // Note: utoipa replaces :: with . in schema names
    const ERROR_SCHEMA_NAME: &str = "crate.domain.Error";
    const USER_SCHEMA_NAME: &str = "crate.domain.User";

    /// Assert that an Object schema contains a field with the given name.
    ///
    /// Handles inline Object schemas. Fails with a diagnostic message if the
    /// schema is a `$ref`, a combinator (`AllOf`, `OneOf`, `AnyOf`), or another
    /// non-Object type, since those require different inspection strategies.
    fn assert_object_schema_has_field(schema: &RefOr<Schema>, schema_name: &str, field: &str) {
        match schema {
            RefOr::T(Schema::Object(obj)) => {
                assert!(
                    obj.properties.contains_key(field),
                    "schema '{schema_name}' should have field '{field}'"
                );
            }
            RefOr::Ref(reference) => {
                panic!(
                    "schema '{schema_name}' is a $ref to '{}'; \
                     resolve the reference before inspecting properties",
                    reference.ref_location
                );
            }
            RefOr::T(Schema::AllOf(_)) => {
                panic!(
                    "schema '{schema_name}' is an AllOf combinator; \
                     inspect composed schemas individually"
                );
            }
            RefOr::T(Schema::OneOf(_)) => {
                panic!(
                    "schema '{schema_name}' is a OneOf combinator; \
                     inspect variant schemas individually"
                );
            }
            RefOr::T(Schema::AnyOf(_)) => {
                panic!(
                    "schema '{schema_name}' is an AnyOf combinator; \
                     inspect variant schemas individually"
                );
            }
            RefOr::T(Schema::Array(_)) => {
                panic!("schema '{schema_name}' is an Array, not an Object");
            }
            // Schema is non-exhaustive; catch future variants
            _ => panic!("schema '{schema_name}' has unexpected type"),
        }
    }

    #[test]
    fn openapi_error_schema_has_required_fields() {
        let doc = ApiDoc::openapi();
        let schemas = &doc.components.as_ref().expect("components").schemas;
        let error_schema = schemas.get(ERROR_SCHEMA_NAME).expect("Error schema");

        assert_object_schema_has_field(error_schema, ERROR_SCHEMA_NAME, "code");
        assert_object_schema_has_field(error_schema, ERROR_SCHEMA_NAME, "message");
    }

    #[test]
    fn openapi_user_schema_has_required_fields() {
        let doc = ApiDoc::openapi();
        let schemas = &doc.components.as_ref().expect("components").schemas;
        let user_schema = schemas.get(USER_SCHEMA_NAME).expect("User schema");

        assert_object_schema_has_field(user_schema, USER_SCHEMA_NAME, "id");
        assert_object_schema_has_field(user_schema, USER_SCHEMA_NAME, "display_name");
    }

    #[test]
    fn openapi_user_id_has_uuid_format() {
        use utoipa::openapi::schema::SchemaFormat;

        let doc = ApiDoc::openapi();
        let schemas = &doc.components.as_ref().expect("components").schemas;
        let user_schema = schemas.get(USER_SCHEMA_NAME).expect("User schema");
        let obj = unwrap_object_schema(user_schema, USER_SCHEMA_NAME);

        let id_prop = obj.properties.get("id").expect("id property exists");
        let id_obj = unwrap_object_schema(id_prop, "id");

        // Schema format is set via #[schema(format = "uuid")] which produces Custom variant
        assert!(
            matches!(&id_obj.format, Some(SchemaFormat::Custom(s)) if s == "uuid"),
            "id should have format=uuid"
        );
    }

    #[test]
    fn openapi_user_display_name_has_constraints() {
        let doc = ApiDoc::openapi();
        let schemas = &doc.components.as_ref().expect("components").schemas;
        let user_schema = schemas.get(USER_SCHEMA_NAME).expect("User schema");
        let obj = unwrap_object_schema(user_schema, USER_SCHEMA_NAME);

        let display_name_prop = obj
            .properties
            .get("display_name")
            .expect("display_name property exists");
        let display_name_obj = unwrap_object_schema(display_name_prop, "display_name");

        assert_eq!(
            display_name_obj.min_length,
            Some(3),
            "display_name should have min_length=3"
        );
        assert_eq!(
            display_name_obj.max_length,
            Some(32),
            "display_name should have max_length=32"
        );
        assert_eq!(
            display_name_obj.pattern.as_deref(),
            Some("^[A-Za-z0-9_ ]+$"),
            "display_name should have pattern constraint"
        );
    }
}
