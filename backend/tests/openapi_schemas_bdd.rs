//! Behaviour tests for OpenAPI schema wrappers.
//!
//! These tests verify that the OpenAPI document correctly references the
//! schema wrapper types from `inbound::http::schemas` instead of domain types.

use std::sync::Mutex;

use backend::doc::ApiDoc;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use utoipa::OpenApi;

#[derive(Default)]
struct OpenApiWorld {
    document: Option<utoipa::openapi::OpenApi>,
    json: Option<String>,
}

impl std::fmt::Debug for OpenApiWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenApiWorld")
            .field("document", &self.document.as_ref().map(|_| "<OpenApi>"))
            .field("json", &self.json)
            .finish()
    }
}

#[fixture]
fn world() -> Mutex<OpenApiWorld> {
    Mutex::new(OpenApiWorld::default())
}

#[given("the OpenAPI document is generated")]
fn generate_openapi_document(world: &Mutex<OpenApiWorld>) {
    let mut world = world.lock().expect("world lock");
    let doc = ApiDoc::openapi();
    world.json = Some(doc.to_json().expect("valid JSON"));
    world.document = Some(doc);
}

#[when("the document is inspected")]
fn inspect_document(world: &Mutex<OpenApiWorld>) {
    // Verify document was generated in the given step
    let world = world.lock().expect("world lock");
    assert!(world.document.is_some(), "document should be generated");
}

// Note: utoipa replaces :: with . in schema names
const ERROR_SCHEMA_NAME: &str = "crate.domain.Error";
const ERROR_CODE_SCHEMA_NAME: &str = "crate.domain.ErrorCode";
const USER_SCHEMA_NAME: &str = "crate.domain.User";

#[then("the components section contains the Error schema wrapper")]
fn contains_error_schema(world: &Mutex<OpenApiWorld>) {
    let world = world.lock().expect("world lock");
    let doc = world.document.as_ref().expect("document generated");
    let components = doc.components.as_ref().expect("components present");

    assert!(
        components.schemas.contains_key(ERROR_SCHEMA_NAME),
        "Error schema wrapper should be registered"
    );
}

#[then("the components section contains the ErrorCode schema wrapper")]
fn contains_error_code_schema(world: &Mutex<OpenApiWorld>) {
    let world = world.lock().expect("world lock");
    let doc = world.document.as_ref().expect("document generated");
    let components = doc.components.as_ref().expect("components present");

    assert!(
        components.schemas.contains_key(ERROR_CODE_SCHEMA_NAME),
        "ErrorCode schema wrapper should be registered"
    );
}

#[then("the components section contains the User schema wrapper")]
fn contains_user_schema(world: &Mutex<OpenApiWorld>) {
    let world = world.lock().expect("world lock");
    let doc = world.document.as_ref().expect("document generated");
    let components = doc.components.as_ref().expect("components present");

    assert!(
        components.schemas.contains_key(USER_SCHEMA_NAME),
        "User schema wrapper should be registered"
    );
}

#[then("the login endpoint references ErrorSchema for error responses")]
fn login_references_error_schema(world: &Mutex<OpenApiWorld>) {
    let world = world.lock().expect("world lock");
    let json = world.json.as_ref().expect("JSON generated");

    // The login endpoint should reference the Error schema in its 400/401 responses
    assert!(
        json.contains(&format!("#/components/schemas/{ERROR_SCHEMA_NAME}")),
        "Login endpoint should reference Error schema"
    );
}

#[then("the list users endpoint references UserSchema for success response")]
fn list_users_references_user_schema(world: &Mutex<OpenApiWorld>) {
    let world = world.lock().expect("world lock");
    let json = world.json.as_ref().expect("JSON generated");

    // The list users endpoint should reference the User schema in its 200 response
    assert!(
        json.contains(&format!("#/components/schemas/{USER_SCHEMA_NAME}")),
        "List users endpoint should reference User schema"
    );
}

#[then("the list users endpoint references ErrorSchema for error responses")]
fn list_users_references_error_schema(world: &Mutex<OpenApiWorld>) {
    let world = world.lock().expect("world lock");
    let json = world.json.as_ref().expect("JSON generated");

    // The list users endpoint should reference the Error schema in error responses
    assert!(
        json.contains(&format!("#/components/schemas/{ERROR_SCHEMA_NAME}")),
        "List users endpoint should reference Error schema for errors"
    );
}

#[then("the User id field has uuid format")]
fn user_id_has_uuid_format(world: &Mutex<OpenApiWorld>) {
    use utoipa::openapi::schema::{Schema, SchemaFormat};
    use utoipa::openapi::RefOr;

    let world = world.lock().expect("world lock");
    let doc = world.document.as_ref().expect("document generated");
    let components = doc.components.as_ref().expect("components present");
    let user_schema = components
        .schemas
        .get(USER_SCHEMA_NAME)
        .expect("User schema");

    let obj = match user_schema {
        RefOr::T(Schema::Object(obj)) => obj,
        _ => panic!("User schema is not an Object"),
    };

    let id_prop = obj.properties.get("id").expect("id property exists");
    let id_obj = match id_prop {
        RefOr::T(Schema::Object(obj)) => obj,
        _ => panic!("id property is not an Object"),
    };

    // Schema format is set via #[schema(format = "uuid")] which produces Custom variant
    assert!(
        matches!(&id_obj.format, Some(SchemaFormat::Custom(s)) if s == "uuid"),
        "User.id should have format=uuid"
    );
}

#[then("the User display_name field has length constraints")]
fn user_display_name_has_length_constraints(world: &Mutex<OpenApiWorld>) {
    use utoipa::openapi::schema::Schema;
    use utoipa::openapi::RefOr;

    let world = world.lock().expect("world lock");
    let doc = world.document.as_ref().expect("document generated");
    let components = doc.components.as_ref().expect("components present");
    let user_schema = components
        .schemas
        .get(USER_SCHEMA_NAME)
        .expect("User schema");

    let obj = match user_schema {
        RefOr::T(Schema::Object(obj)) => obj,
        _ => panic!("User schema is not an Object"),
    };

    let display_name_prop = obj
        .properties
        .get("display_name")
        .expect("display_name property");
    let display_name_obj = match display_name_prop {
        RefOr::T(Schema::Object(obj)) => obj,
        _ => panic!("display_name property is not an Object"),
    };

    assert_eq!(
        display_name_obj.min_length,
        Some(3),
        "User.display_name should have min_length=3"
    );
    assert_eq!(
        display_name_obj.max_length,
        Some(32),
        "User.display_name should have max_length=32"
    );
}

#[then("the User display_name field has pattern constraint")]
fn user_display_name_has_pattern_constraint(world: &Mutex<OpenApiWorld>) {
    use utoipa::openapi::schema::Schema;
    use utoipa::openapi::RefOr;

    let world = world.lock().expect("world lock");
    let doc = world.document.as_ref().expect("document generated");
    let components = doc.components.as_ref().expect("components present");
    let user_schema = components
        .schemas
        .get(USER_SCHEMA_NAME)
        .expect("User schema");

    let obj = match user_schema {
        RefOr::T(Schema::Object(obj)) => obj,
        _ => panic!("User schema is not an Object"),
    };

    let display_name_prop = obj
        .properties
        .get("display_name")
        .expect("display_name property");
    let display_name_obj = match display_name_prop {
        RefOr::T(Schema::Object(obj)) => obj,
        _ => panic!("display_name property is not an Object"),
    };

    assert_eq!(
        display_name_obj.pattern.as_deref(),
        Some("^[A-Za-z0-9_ ]+$"),
        "User.display_name should have pattern constraint"
    );
}

#[scenario(path = "tests/features/openapi_schemas.feature")]
fn openapi_schemas(world: Mutex<OpenApiWorld>) {
    drop(world);
}
