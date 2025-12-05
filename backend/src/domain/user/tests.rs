//! Tests for the domain user model.

use super::*;
use rstest::{fixture, rstest};
use rstest_bdd_macros::{given, then, when};
use serde_json::json;

const VALID_ID: &str = "3fa85f64-5717-4562-b3fc-2c963f66afa6";

#[fixture]
fn valid_id() -> String {
    VALID_ID.to_owned()
}

#[fixture]
fn valid_display_name() -> String {
    "Ada Lovelace".to_owned()
}

#[rstest]
fn accepts_minimum_length(valid_id: String) {
    let name = "a".repeat(DISPLAY_NAME_MIN);
    let result = User::try_from_strings(valid_id, name.clone());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().display_name().as_ref(), name);
}

#[rstest]
fn accepts_maximum_length(valid_id: String) {
    let name = "a".repeat(DISPLAY_NAME_MAX);
    let result = User::try_from_strings(valid_id, name.clone());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().display_name().as_ref(), name);
}

#[rstest]
fn new_panics_when_invalid_id() {
    let result = std::panic::catch_unwind(|| User::from_strings("", "Ada"));
    assert!(result.is_err());
}

#[rstest]
fn try_new_rejects_invalid_uuid(valid_display_name: String) {
    let result = User::try_from_strings("not-a-uuid", valid_display_name);
    assert!(matches!(result, Err(UserValidationError::InvalidId)));
}

#[rstest]
fn try_new_rejects_uuid_with_whitespace(valid_display_name: String) {
    let id = format!(" {VALID_ID} ");
    let result = User::try_from_strings(id, valid_display_name);
    assert!(matches!(result, Err(UserValidationError::InvalidId)));
}

#[rstest]
fn try_new_rejects_empty_display_name(valid_id: String) {
    let result = User::try_from_strings(valid_id, "   ");
    assert!(matches!(result, Err(UserValidationError::EmptyDisplayName)));
}

#[rstest]
fn try_new_rejects_too_short_display_name(valid_id: String) {
    let result = User::try_from_strings(valid_id, "ab");
    assert!(matches!(
        result,
        Err(UserValidationError::DisplayNameTooShort { min }) if min == DISPLAY_NAME_MIN
    ));
}

#[rstest]
fn try_new_rejects_too_long_display_name(valid_id: String) {
    let long = "a".repeat(DISPLAY_NAME_MAX + 1);
    let result = User::try_from_strings(valid_id, long);
    assert!(matches!(
        result,
        Err(UserValidationError::DisplayNameTooLong { max }) if max == DISPLAY_NAME_MAX
    ));
}

#[rstest]
fn try_new_rejects_invalid_characters(valid_id: String) {
    let result = User::try_from_strings(valid_id, "bad$char");
    assert!(matches!(
        result,
        Err(UserValidationError::DisplayNameInvalidCharacters)
    ));
}

#[rstest]
fn try_new_accepts_valid_inputs(valid_id: String, valid_display_name: String) {
    let user =
        User::try_from_strings(valid_id.clone(), valid_display_name.clone()).expect("valid inputs");
    assert_eq!(user.id().as_ref(), valid_id);
    assert_eq!(user.display_name().as_ref(), valid_display_name);
}

#[rstest]
fn display_name_allows_alphanumerics_spaces_and_underscores(valid_id: String) {
    let name = "Alice_Bob 123";
    let user = User::try_from_strings(valid_id, name).expect("valid name");
    assert_eq!(user.display_name().as_ref(), name);
}

#[rstest]
fn display_name_rejects_forbidden_characters(valid_id: String) {
    let result = User::try_from_strings(valid_id, "bad$char");
    assert!(matches!(
        result,
        Err(UserValidationError::DisplayNameInvalidCharacters)
    ));
}

#[rstest]
fn serde_round_trips_alias(valid_id: String, valid_display_name: String) {
    let camel = json!({
        "id": valid_id.clone(),
        "displayName": valid_display_name.clone()
    });
    let snake = json!({
        "id": valid_id.clone(),
        "display_name": valid_display_name.clone()
    });
    let from_camel: User = serde_json::from_value(camel).expect("camelCase");
    let from_snake: User = serde_json::from_value(snake).expect("snake_case");
    assert_eq!(from_camel, from_snake);

    let value = serde_json::to_value(from_snake).expect("serialise to JSON");
    assert_eq!(
        value.get("displayName").and_then(|v| v.as_str()),
        Some(valid_display_name.as_str())
    );
    assert!(value.get("display_name").is_none());
}

#[given("a valid user payload")]
fn a_valid_user_payload(valid_id: String, valid_display_name: String) -> (String, String) {
    (valid_id, valid_display_name)
}

#[when("the user is constructed")]
fn the_user_is_constructed(payload: (String, String)) -> Result<User, UserValidationError> {
    User::try_from_strings(payload.0, payload.1)
}

#[then("the user is returned")]
fn the_user_is_returned(result: Result<User, UserValidationError>, valid_id: String) {
    let user = result.expect("user should be created");
    assert_eq!(user.id().as_ref(), valid_id);
}

#[rstest]
fn constructing_a_user_happy_path(valid_display_name: String, valid_id: String) {
    let payload = a_valid_user_payload(valid_id.clone(), valid_display_name);
    let result = the_user_is_constructed(payload);
    the_user_is_returned(result, valid_id);
}

#[given("a payload with an empty display name")]
fn a_payload_with_an_empty_display_name(valid_id: String) -> (String, String) {
    (valid_id, "   ".to_owned())
}

#[then("user construction fails")]
fn user_construction_fails(result: Result<User, UserValidationError>) {
    assert!(matches!(result, Err(UserValidationError::EmptyDisplayName)));
}

#[rstest]
fn constructing_a_user_unhappy_path(valid_id: String) {
    let payload = a_payload_with_an_empty_display_name(valid_id);
    let result = the_user_is_constructed(payload);
    user_construction_fails(result);
}
