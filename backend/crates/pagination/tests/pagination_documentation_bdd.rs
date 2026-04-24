//! BDD tests verifying documentation invariants.

mod common;

use base64::Engine as _;
use common::{Cursor, CursorError, FixtureKey, PageParams, PageParamsError, World};
use pagination::{DEFAULT_LIMIT, MAX_LIMIT};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

#[fixture]
fn world() -> World {
    World::default()
}

// Documentation invariant step definitions

#[given("pagination parameters without a limit")]
#[expect(
    clippy::expect_used,
    reason = "BDD steps use expect for clear failures"
)]
fn pagination_parameters_without_a_limit(world: &World) {
    let params = PageParams::new(None, None).expect("default params should be valid");
    world.page_params.set(params);
}

#[given("pagination parameters with limit {limit:u64}")]
fn pagination_parameters_with_limit(world: &World, limit: u64) {
    common::set_page_params_with_limit(world, limit);
}

#[given("an invalid base64 cursor token {token}")]
fn an_invalid_base64_cursor_token(world: &World, token: String) {
    world.cursor_token.set(token);
}

#[given("a base64url token containing invalid JSON")]
fn a_base64url_token_containing_invalid_json(world: &World) {
    let invalid_json = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"{not-json");
    world.cursor_token.set(invalid_json);
}

#[given("cursor decoding errors of different variants")]
fn cursor_decoding_errors_of_different_variants(world: &World) {
    let mut errors = Vec::new();

    let invalid_base64_result = Cursor::<FixtureKey>::decode("not!valid");
    if let Err(e) = invalid_base64_result {
        errors.push(e);
    }

    let invalid_json = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"{not-json");
    let deserialize_result = Cursor::<FixtureKey>::decode(&invalid_json);
    if let Err(e) = deserialize_result {
        errors.push(e);
    }

    assert_eq!(
        errors.len(),
        2,
        "expected both InvalidBase64 and Deserialize errors to be collected"
    );

    world.cursor_errors.set(errors);
}

#[when("the cursor is decoded")]
#[expect(
    clippy::expect_used,
    reason = "BDD steps use expect for clear failures"
)]
fn the_cursor_is_decoded(world: &World) {
    let token = world
        .cursor_token
        .get()
        .expect("cursor token should be set")
        .clone();
    world
        .decode_result
        .set(Cursor::<FixtureKey>::decode(&token));
}

#[when("pagination parameters are created with limit {limit:u64}")]
#[expect(
    clippy::expect_used,
    reason = "BDD steps use expect for clear failures"
)]
fn pagination_parameters_are_created_with_limit(world: &World, limit: u64) {
    let requested_limit = usize::try_from(limit).expect("fixture limit should fit usize");
    let result = PageParams::new(None, Some(requested_limit));
    world.page_params_result.set(result);
}

#[then("the normalized limit equals DEFAULT_LIMIT")]
#[expect(
    clippy::expect_used,
    reason = "BDD steps use expect for clear failures"
)]
fn the_normalized_limit_equals_default_limit(world: &World) {
    let params = world.page_params.get().expect("page params should be set");
    assert_eq!(params.limit(), DEFAULT_LIMIT);
}

#[then("the normalized limit equals MAX_LIMIT")]
#[expect(
    clippy::expect_used,
    reason = "BDD steps use expect for clear failures"
)]
fn the_normalized_limit_equals_max_limit(world: &World) {
    let params = world.page_params.get().expect("page params should be set");
    assert_eq!(params.limit(), MAX_LIMIT);
}

#[then("page parameter creation fails with InvalidLimit error")]
#[expect(
    clippy::expect_used,
    reason = "BDD steps use expect for clear failures"
)]
fn page_parameter_creation_fails_with_invalid_limit_error(world: &World) {
    let result = world
        .page_params_result
        .get()
        .expect("page params result should be set");

    assert!(matches!(result, Err(PageParamsError::InvalidLimit)));
}

#[then("decoding fails with InvalidBase64 error")]
#[expect(
    clippy::expect_used,
    reason = "BDD steps use expect for clear failures"
)]
fn decoding_fails_with_invalid_base64_error(world: &World) {
    let result = world
        .decode_result
        .get()
        .expect("decode result should be set");

    assert!(matches!(result, Err(CursorError::InvalidBase64 { .. })));
}

#[then("decoding fails with Deserialize error")]
#[expect(
    clippy::expect_used,
    reason = "BDD steps use expect for clear failures"
)]
fn decoding_fails_with_deserialize_error(world: &World) {
    let result = world
        .decode_result
        .get()
        .expect("decode result should be set");

    assert!(matches!(result, Err(CursorError::Deserialize { .. })));
}

#[then("each error display string contains a descriptive message")]
#[expect(
    clippy::expect_used,
    reason = "BDD steps use expect for clear failures"
)]
fn each_error_display_string_contains_a_descriptive_message(world: &World) {
    let errors = world
        .cursor_errors
        .get()
        .expect("cursor errors should be set");

    for error in &errors {
        let display = format!("{error}");
        let has_descriptive_content = display.contains("base64")
            || display.contains("deserialize")
            || display.contains("decode")
            || display.contains("invalid")
            || display.contains("JSON");
        assert!(
            has_descriptive_content,
            "error display string should contain descriptive keywords like 'base64', 'deserialize', 'decode', 'invalid', or 'JSON'; got: {display}"
        );
    }
}

#[scenario(path = "tests/features/pagination_documentation.feature")]
fn pagination_documentation_invariants(world: World) {
    drop(world);
}
