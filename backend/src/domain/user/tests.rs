//! Tests for the domain user model.

use super::*;
use rstest::{fixture, rstest};
use rstest_bdd_macros::{given, then, when};
use serde_json::json;

const VALID_ID: &str = "3fa85f64-5717-4562-b3fc-2c963f66afa6";

#[derive(Debug, Clone)]
struct TestUserId(String);

impl TestUserId {
    fn valid() -> Self {
        Self(VALID_ID.to_owned())
    }

    fn invalid() -> Self {
        Self("not-a-uuid".to_owned())
    }
}

impl From<&str> for TestUserId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl AsRef<str> for TestUserId {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl From<TestUserId> for String {
    fn from(value: TestUserId) -> Self {
        value.0
    }
}

#[derive(Debug, Clone)]
struct TestDisplayName(String);

impl TestDisplayName {
    fn valid() -> Self {
        Self("Ada Lovelace".to_owned())
    }

    fn too_short() -> Self {
        Self("ab".to_owned())
    }

    fn too_long() -> Self {
        Self("a".repeat(DISPLAY_NAME_MAX + 1))
    }

    fn with_invalid_chars() -> Self {
        Self("bad$char".to_owned())
    }
}

impl From<&str> for TestDisplayName {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl AsRef<str> for TestDisplayName {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl From<TestDisplayName> for String {
    fn from(value: TestDisplayName) -> Self {
        value.0
    }
}

#[fixture]
fn valid_id() -> TestUserId {
    TestUserId::valid()
}

#[fixture]
fn valid_display_name() -> TestDisplayName {
    TestDisplayName::valid()
}

#[rstest]
fn accepts_minimum_length(valid_id: TestUserId) {
    let name = "a".repeat(DISPLAY_NAME_MIN);
    let result = User::try_from_strings(valid_id.as_ref(), name.clone());
    assert!(result.is_ok());
    assert_eq!(
        result
            .expect("valid display name at boundary")
            .display_name()
            .as_ref(),
        name
    );
}

#[rstest]
fn accepts_maximum_length(valid_id: TestUserId) {
    let name = "a".repeat(DISPLAY_NAME_MAX);
    let result = User::try_from_strings(valid_id.as_ref(), name.clone());
    assert!(result.is_ok());
    assert_eq!(
        result
            .expect("valid display name at boundary")
            .display_name()
            .as_ref(),
        name
    );
}

#[rstest]
fn new_panics_when_invalid_id() {
    let result = std::panic::catch_unwind(|| User::from_strings("", "Ada"));
    assert!(result.is_err());
}

#[rstest]
fn try_new_rejects_invalid_uuid(valid_display_name: TestDisplayName) {
    let result =
        User::try_from_strings(TestUserId::invalid().as_ref(), valid_display_name.as_ref());
    assert!(matches!(result, Err(UserValidationError::InvalidId)));
}

#[rstest]
fn try_new_rejects_uuid_with_whitespace(valid_display_name: TestDisplayName) {
    let id = format!(" {VALID_ID} ");
    let result = User::try_from_strings(id, valid_display_name.as_ref());
    assert!(matches!(result, Err(UserValidationError::InvalidId)));
}

#[rstest]
fn try_new_rejects_empty_display_name(valid_id: TestUserId) {
    let result = User::try_from_strings(valid_id.as_ref(), "   ");
    assert!(matches!(result, Err(UserValidationError::EmptyDisplayName)));
}

#[rstest]
fn try_new_rejects_too_short_display_name(valid_id: TestUserId) {
    let display = TestDisplayName::too_short();
    let result = User::try_from_strings(valid_id.as_ref(), display.as_ref());
    assert!(matches!(
        result,
        Err(UserValidationError::DisplayNameTooShort { min }) if min == DISPLAY_NAME_MIN
    ));
}

#[rstest]
fn try_new_rejects_too_long_display_name(valid_id: TestUserId) {
    let display = TestDisplayName::too_long();
    let result = User::try_from_strings(valid_id.as_ref(), display.as_ref());
    assert!(matches!(
        result,
        Err(UserValidationError::DisplayNameTooLong { max }) if max == DISPLAY_NAME_MAX
    ));
}

#[rstest]
fn try_new_accepts_valid_inputs(valid_id: TestUserId, valid_display_name: TestDisplayName) {
    let user = User::try_from_strings(valid_id.as_ref(), valid_display_name.as_ref())
        .expect("valid inputs");
    assert_eq!(user.id().as_ref(), valid_id.as_ref());
    assert_eq!(user.display_name().as_ref(), valid_display_name.as_ref());
}

#[rstest]
fn user_id_from_uuid_avoids_round_trip_parse() {
    let uuid = uuid::Uuid::parse_str(VALID_ID).expect("valid UUID");
    let user_id = UserId::from_uuid(uuid);

    assert_eq!(user_id.as_uuid(), &uuid);
    assert_eq!(user_id.as_ref(), VALID_ID);
}

#[rstest]
fn display_name_allows_alphanumerics_spaces_and_underscores(valid_id: TestUserId) {
    let name = "Alice_Bob 123";
    let user = User::try_from_strings(valid_id.as_ref(), name).expect("valid name");
    assert_eq!(user.display_name().as_ref(), name);
}

#[rstest]
fn display_name_rejects_forbidden_characters(valid_id: TestUserId) {
    let display = TestDisplayName::with_invalid_chars();
    let result = User::try_from_strings(valid_id.as_ref(), display.as_ref());
    assert!(matches!(
        result,
        Err(UserValidationError::DisplayNameInvalidCharacters)
    ));
}

#[rstest]
fn serde_round_trips_alias(valid_id: TestUserId, valid_display_name: TestDisplayName) {
    let camel = json!({
        "id": valid_id.as_ref(),
        "displayName": valid_display_name.as_ref()
    });
    let snake = json!({
        "id": valid_id.as_ref(),
        "display_name": valid_display_name.as_ref()
    });
    let from_camel: User = serde_json::from_value(camel).expect("camelCase");
    let from_snake: User = serde_json::from_value(snake).expect("snake_case");
    assert_eq!(from_camel, from_snake);

    let value = serde_json::to_value(from_snake).expect("serialise to JSON");
    assert_eq!(
        value.get("displayName").and_then(|v| v.as_str()),
        Some(valid_display_name.as_ref())
    );
    assert!(value.get("display_name").is_none());
}

#[given("a valid user payload")]
fn a_valid_user_payload(
    valid_id: TestUserId,
    valid_display_name: TestDisplayName,
) -> (TestUserId, TestDisplayName) {
    (valid_id, valid_display_name)
}

#[when("the user is constructed")]
fn the_user_is_constructed(
    payload: (TestUserId, TestDisplayName),
) -> Result<User, UserValidationError> {
    let (id, display_name) = payload;
    User::try_from_strings(id.as_ref(), display_name.as_ref())
}

#[then("the user is returned")]
fn the_user_is_returned(result: Result<User, UserValidationError>, valid_id: TestUserId) {
    let user = result.expect("user should be created");
    assert_eq!(user.id().as_ref(), valid_id.as_ref());
}

#[rstest]
fn constructing_a_user_happy_path(valid_display_name: TestDisplayName, valid_id: TestUserId) {
    let payload = a_valid_user_payload(valid_id.clone(), valid_display_name);
    let result = the_user_is_constructed(payload);
    the_user_is_returned(result, valid_id);
}

#[given("a payload with an empty display name")]
fn a_payload_with_an_empty_display_name(valid_id: TestUserId) -> (TestUserId, TestDisplayName) {
    (valid_id, TestDisplayName::from("   "))
}

#[then("user construction fails")]
fn user_construction_fails(result: Result<User, UserValidationError>) {
    assert!(matches!(result, Err(UserValidationError::EmptyDisplayName)));
}

#[rstest]
fn constructing_a_user_unhappy_path(valid_id: TestUserId) {
    let payload = a_payload_with_an_empty_display_name(valid_id);
    let result = the_user_is_constructed(payload);
    user_construction_fails(result);
}
