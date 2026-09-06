//! OpenAPI documentation configuration.
//!
//! This module defines the [`ApiDoc`] struct which generates the OpenAPI
//! specification for the REST API. It registers:
//!
//! - **Paths**: All HTTP endpoints from the inbound layer (users, health)
//! - **Schemas**: Domain type wrappers ([`ErrorSchema`], [`ErrorCodeSchema`],
//!   [`UserSchema`]) for shared types that use external schema registration
//! - **Security**: Session cookie authentication scheme
//!
//! The generated specification is used by Swagger UI (debug builds) and
//! exported via `cargo run --bin openapi-dump` for external tooling.

use crate::inbound::http::admin_enrichment::{
    EnrichmentProvenanceRecordBody, ListEnrichmentProvenanceQuery,
    ListEnrichmentProvenanceResponseBody, ProvenanceBoundsBody,
};
use crate::inbound::http::catalogue::{DescriptorsResponse, ExploreCatalogueResponse};
use crate::inbound::http::offline::{
    BoundsBody, DeleteOfflineBundleResponseBody, ListOfflineBundlesQuery,
    ListOfflineBundlesResponseBody, OfflineBundleResponse, UpsertOfflineBundleRequestBody,
    UpsertOfflineBundleResponseBody, ZoomRangeBody,
};
use crate::inbound::http::schemas::{
    ErrorCodeSchema, ErrorSchema, InterestThemeIdSchema, UserInterestsSchema, UserSchema,
};
use crate::inbound::http::users_pagination::{PaginatedUsersResponse, PaginationLinksSchema};
use crate::inbound::http::walk_sessions::{
    CreateWalkSessionRequestBody, CreateWalkSessionResponseBody, WalkCompletionSummaryResponseBody,
    WalkPrimaryStatBody, WalkSecondaryStatBody,
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
        crate::inbound::http::catalogue::get_explore_catalogue,
        crate::inbound::http::catalogue::get_descriptors,
        crate::inbound::http::admin_enrichment::list_enrichment_provenance,
        crate::inbound::http::offline::list_offline_bundles,
        crate::inbound::http::offline::upsert_offline_bundle,
        crate::inbound::http::offline::delete_offline_bundle,
        crate::inbound::http::walk_sessions::create_walk_session,
        crate::inbound::http::routes::submit_route,
    ),
    components(schemas(
        UserSchema,
        UserInterestsSchema,
        InterestThemeIdSchema,
        PaginationLinksSchema,
        PaginatedUsersResponse,
        ErrorSchema,
        ErrorCodeSchema,
        ExploreCatalogueResponse,
        DescriptorsResponse,
        ListEnrichmentProvenanceQuery,
        ProvenanceBoundsBody,
        EnrichmentProvenanceRecordBody,
        ListEnrichmentProvenanceResponseBody,
        ListOfflineBundlesQuery,
        BoundsBody,
        ZoomRangeBody,
        OfflineBundleResponse,
        ListOfflineBundlesResponseBody,
        UpsertOfflineBundleRequestBody,
        UpsertOfflineBundleResponseBody,
        DeleteOfflineBundleResponseBody,
        WalkPrimaryStatBody,
        WalkSecondaryStatBody,
        WalkCompletionSummaryResponseBody,
        CreateWalkSessionRequestBody,
        CreateWalkSessionResponseBody
    )),
    tags(
        (name = "users", description = "Operations related to users"),
        (name = "routes", description = "Operations related to routes"),
        (name = "health", description = "Endpoints for health checks"),
        (name = "catalogue", description = "Catalogue and descriptor read endpoints"),
        (name = "admin", description = "Admin reporting endpoints"),
        (name = "offline", description = "Offline bundle manifest operations"),
        (name = "walk-sessions", description = "Walk session recording operations")
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
    use crate::domain::ports::{ROUTE_PREFERENCE_MAX_ITEMS, ROUTE_PREFERENCE_MAX_VALUE_BYTES};
    use crate::test_support::openapi::unwrap_object_schema;
    use rstest::{fixture, rstest};
    use serde_json::json;
    use utoipa::OpenApi;
    use utoipa::openapi::Components;
    use utoipa::openapi::RefOr;
    use utoipa::openapi::schema::{AdditionalProperties, Schema};

    // Note: utoipa replaces :: with . in schema names
    const ERROR_SCHEMA_NAME: &str = "crate.domain.Error";
    const ROUTE_PREFERENCES_SCHEMA_NAME: &str = "RoutePreferences";
    const ROUTE_RESPONSE_SCHEMA_NAME: &str = "RouteResponse";
    const USER_SCHEMA_NAME: &str = "crate.domain.User";

    #[fixture]
    fn openapi_components() -> Components {
        match ApiDoc::openapi().components {
            Some(components) => components,
            None => panic!("OpenAPI document should include components"),
        }
    }

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

    #[rstest]
    fn openapi_error_schema_has_required_fields(openapi_components: Components) {
        let schemas = &openapi_components.schemas;
        let error_schema = schemas.get(ERROR_SCHEMA_NAME).expect("Error schema");

        assert_object_schema_has_field(error_schema, ERROR_SCHEMA_NAME, "code");
        assert_object_schema_has_field(error_schema, ERROR_SCHEMA_NAME, "message");
    }

    #[rstest]
    fn openapi_user_schema_has_required_fields(openapi_components: Components) {
        let schemas = &openapi_components.schemas;
        let user_schema = schemas.get(USER_SCHEMA_NAME).expect("User schema");

        assert_object_schema_has_field(user_schema, USER_SCHEMA_NAME, "id");
        assert_object_schema_has_field(user_schema, USER_SCHEMA_NAME, "displayName");
    }

    #[rstest]
    fn openapi_route_preferences_schema_has_expected_fields(openapi_components: Components) {
        let schemas = &openapi_components.schemas;
        let preferences_schema = schemas
            .get(ROUTE_PREFERENCES_SCHEMA_NAME)
            .expect("RoutePreferences schema");

        assert_object_schema_has_field(
            preferences_schema,
            ROUTE_PREFERENCES_SCHEMA_NAME,
            "themeIds",
        );
        assert_object_schema_has_field(
            preferences_schema,
            ROUTE_PREFERENCES_SCHEMA_NAME,
            "interestThemeIds",
        );
        assert_object_schema_has_field(
            preferences_schema,
            ROUTE_PREFERENCES_SCHEMA_NAME,
            "avoidStairs",
        );

        let preferences_object =
            unwrap_object_schema(preferences_schema, ROUTE_PREFERENCES_SCHEMA_NAME);
        assert!(
            matches!(
                preferences_object.additional_properties.as_deref(),
                Some(AdditionalProperties::FreeForm(false))
            ),
            "RoutePreferences should reject unknown fields"
        );
    }

    #[rstest]
    fn openapi_route_preferences_schema_documents_collection_and_byte_limits(
        openapi_components: Components,
    ) {
        let schemas = &openapi_components.schemas;
        let preferences_schema = schemas
            .get(ROUTE_PREFERENCES_SCHEMA_NAME)
            .expect("RoutePreferences schema");
        let preferences_object =
            unwrap_object_schema(preferences_schema, ROUTE_PREFERENCES_SCHEMA_NAME);

        for field in ["themes", "themeIds", "interestThemeIds", "avoid"] {
            let schema = preferences_object
                .properties
                .get(field)
                .unwrap_or_else(|| panic!("{field} property exists"));
            let schema = serde_json::to_value(schema).expect("serializes array schema");

            assert_eq!(
                schema.pointer("/maxItems"),
                Some(&json!(ROUTE_PREFERENCE_MAX_ITEMS)),
                "{field} should limit collection size"
            );
            assert_eq!(
                schema.pointer("/items/x-max-utf8-bytes"),
                Some(&json!(ROUTE_PREFERENCE_MAX_VALUE_BYTES)),
                "{field} items should publish the UTF-8 byte limit"
            );
        }

        let mode_schema = preferences_object
            .properties
            .get("mode")
            .expect("mode property exists");
        let mode_schema = serde_json::to_value(mode_schema).expect("serializes mode schema");
        assert_eq!(
            mode_schema.pointer("/x-max-utf8-bytes"),
            Some(&json!(ROUTE_PREFERENCE_MAX_VALUE_BYTES)),
            "mode should publish the UTF-8 byte limit"
        );
    }

    #[rstest]
    fn openapi_route_response_schema_has_typed_metadata(openapi_components: Components) {
        let schemas = &openapi_components.schemas;
        let response_schema = schemas
            .get(ROUTE_RESPONSE_SCHEMA_NAME)
            .expect("RouteResponse schema");
        let response_object = unwrap_object_schema(response_schema, ROUTE_RESPONSE_SCHEMA_NAME);

        let request_id_schema = response_object
            .properties
            .get("requestId")
            .expect("requestId property exists");
        let request_id_schema =
            serde_json::to_value(request_id_schema).expect("serializes requestId schema");
        assert_eq!(
            request_id_schema.pointer("/format"),
            Some(&json!("uuid")),
            "requestId should have UUID format"
        );

        let status_schema = response_object
            .properties
            .get("status")
            .expect("status property exists");
        let status_schema = serde_json::to_value(status_schema).expect("serializes status schema");
        assert_eq!(
            status_schema.pointer("/enum"),
            Some(&json!(["accepted", "replayed"])),
            "status should enumerate submission outcomes"
        );
    }

    #[rstest]
    fn openapi_user_id_has_uuid_format(openapi_components: Components) {
        use utoipa::openapi::schema::SchemaFormat;

        let schemas = &openapi_components.schemas;
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

    #[rstest]
    fn openapi_user_display_name_has_constraints(openapi_components: Components) {
        let schemas = &openapi_components.schemas;
        let user_schema = schemas.get(USER_SCHEMA_NAME).expect("User schema");
        let obj = unwrap_object_schema(user_schema, USER_SCHEMA_NAME);

        let display_name_prop = obj
            .properties
            .get("displayName")
            .expect("displayName property exists");
        let display_name_obj = unwrap_object_schema(display_name_prop, "displayName");

        assert_eq!(
            display_name_obj.min_length,
            Some(3),
            "displayName should have min_length=3"
        );
        assert_eq!(
            display_name_obj.max_length,
            Some(32),
            "displayName should have max_length=32"
        );
        assert_eq!(
            display_name_obj.pattern.as_deref(),
            Some("^[A-Za-z0-9_ ]+$"),
            "displayName should have pattern constraint"
        );
    }
}
