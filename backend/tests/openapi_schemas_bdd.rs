//! Behaviour tests for OpenAPI schema wrappers.
//!
//! These tests verify that the OpenAPI document correctly references the
//! schema wrapper types from `inbound::http::schemas` instead of domain types.
use std::sync::Mutex;

use backend::doc::ApiDoc;
use backend::test_support::openapi::{get_property, unwrap_object_schema};
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
const INTEREST_THEME_ID_SCHEMA_NAME: &str = "crate.domain.InterestThemeId";
const USER_SCHEMA_NAME: &str = "crate.domain.User";
const USER_INTERESTS_SCHEMA_NAME: &str = "crate.domain.UserInterests";

/// Navigate into a User property's object schema and invoke a closure.
///
/// This helper reduces boilerplate when asserting constraints on User schema
/// properties (e.g., `id`, `displayName`). It handles the traversal from the
/// OpenAPI document root down to a specific property's object schema.
///
/// # Parameters
///
/// - `world`: Mutex guarding the test world containing the OpenAPI document.
/// - `property_name`: Name of the User property to inspect (e.g., `"id"`).
/// - `f`: Closure receiving the property's `Object` schema for assertions.
///
/// # Usage Pattern
///
/// 1. Locks the world mutex to access the OpenAPI document
/// 2. Extracts components and locates the User schema by name
/// 3. Unwraps the User schema to an `Object` (panics with diagnostics if not)
/// 4. Retrieves the named property and unwraps it to an `Object`
/// 5. Invokes the closure with the property's object schema
///
/// # Example
///
/// ```ignore
/// with_user_property_object_schema(world, "displayName", |obj| {
///     assert_eq!(obj.min_length, Some(3));
/// });
/// ```
fn with_user_property_object_schema<F>(world: &Mutex<OpenApiWorld>, property_name: &str, f: F)
where
    F: FnOnce(&utoipa::openapi::schema::Object),
{
    let world = world.lock().expect("world lock");
    let doc = world.document.as_ref().expect("document generated");
    let components = doc.components.as_ref().expect("components present");
    let user_schema = components
        .schemas
        .get(USER_SCHEMA_NAME)
        .expect("User schema");

    let obj = unwrap_object_schema(user_schema, USER_SCHEMA_NAME);
    let property = get_property(obj, property_name);
    let property_obj = unwrap_object_schema(property, property_name);

    f(property_obj);
}

fn assert_schema_registered(world: &Mutex<OpenApiWorld>, schema_name: &str, label: &str) {
    let world = world.lock().expect("world lock");
    let doc = world.document.as_ref().expect("document generated");
    let components = doc.components.as_ref().expect("components present");

    assert!(
        components.schemas.contains_key(schema_name),
        "{label} schema wrapper should be registered"
    );
}

fn assert_json_references_schema(world: &Mutex<OpenApiWorld>, schema_name: &str, label: &str) {
    let world = world.lock().expect("world lock");
    let json = world.json.as_ref().expect("JSON generated");

    assert!(
        json.contains(&format!("#/components/schemas/{schema_name}")),
        "{label} should reference {schema_name}"
    );
}

#[then("the components section contains the Error schema wrapper")]
fn contains_error_schema(world: &Mutex<OpenApiWorld>) {
    assert_schema_registered(world, ERROR_SCHEMA_NAME, "Error");
}

#[then("the components section contains the ErrorCode schema wrapper")]
fn contains_error_code_schema(world: &Mutex<OpenApiWorld>) {
    assert_schema_registered(world, ERROR_CODE_SCHEMA_NAME, "ErrorCode");
}

#[then("the components section contains the User schema wrapper")]
fn contains_user_schema(world: &Mutex<OpenApiWorld>) {
    assert_schema_registered(world, USER_SCHEMA_NAME, "User");
}

#[then("the components section contains the InterestThemeId schema wrapper")]
fn contains_interest_theme_id_schema(world: &Mutex<OpenApiWorld>) {
    assert_schema_registered(world, INTEREST_THEME_ID_SCHEMA_NAME, "InterestThemeId");
}

#[then("the components section contains the UserInterests schema wrapper")]
fn contains_user_interests_schema(world: &Mutex<OpenApiWorld>) {
    assert_schema_registered(world, USER_INTERESTS_SCHEMA_NAME, "UserInterests");
}

#[then("the login endpoint references ErrorSchema for error responses")]
fn login_references_error_schema(world: &Mutex<OpenApiWorld>) {
    assert_json_references_schema(world, ERROR_SCHEMA_NAME, "Login endpoint");
}

#[then("the list users endpoint references UserSchema for success response")]
fn list_users_references_user_schema(world: &Mutex<OpenApiWorld>) {
    assert_json_references_schema(world, USER_SCHEMA_NAME, "List users endpoint");
}

#[then("the list users endpoint references ErrorSchema for error responses")]
fn list_users_references_error_schema(world: &Mutex<OpenApiWorld>) {
    assert_json_references_schema(world, ERROR_SCHEMA_NAME, "List users endpoint");
}

#[then("the current user endpoint references UserSchema for success response")]
fn current_user_references_user_schema(world: &Mutex<OpenApiWorld>) {
    assert_json_references_schema(world, USER_SCHEMA_NAME, "Current user endpoint");
}

#[then("the update interests endpoint references UserInterestsSchema")]
fn update_interests_references_user_interests_schema(world: &Mutex<OpenApiWorld>) {
    assert_json_references_schema(
        world,
        USER_INTERESTS_SCHEMA_NAME,
        "Update interests endpoint",
    );
}

#[then("the update interests endpoint references ErrorSchema for error responses")]
fn update_interests_references_error_schema(world: &Mutex<OpenApiWorld>) {
    assert_json_references_schema(world, ERROR_SCHEMA_NAME, "Update interests endpoint");
}

#[then("the User id field has uuid format")]
fn user_id_has_uuid_format(world: &Mutex<OpenApiWorld>) {
    use utoipa::openapi::schema::SchemaFormat;

    with_user_property_object_schema(world, "id", |id_obj| {
        assert!(
            matches!(&id_obj.format, Some(SchemaFormat::Custom(s)) if s == "uuid"),
            "User.id should have format=uuid"
        );
    });
}

#[then("the User displayName field has length constraints")]
fn user_display_name_has_length_constraints(world: &Mutex<OpenApiWorld>) {
    with_user_property_object_schema(world, "displayName", |display_name_obj| {
        assert_eq!(
            display_name_obj.min_length,
            Some(3),
            "User.displayName should have min_length=3"
        );
        assert_eq!(
            display_name_obj.max_length,
            Some(32),
            "User.displayName should have max_length=32"
        );
    });
}

#[then("the User displayName field has pattern constraint")]
fn user_display_name_has_pattern_constraint(world: &Mutex<OpenApiWorld>) {
    with_user_property_object_schema(world, "displayName", |display_name_obj| {
        assert_eq!(
            display_name_obj.pattern.as_deref(),
            Some("^[A-Za-z0-9_ ]+$"),
            "User.displayName should have pattern constraint"
        );
    });
}

#[scenario(path = "tests/features/openapi_schemas.feature")]
fn openapi_schemas(world: Mutex<OpenApiWorld>) {
    drop(world);
}
