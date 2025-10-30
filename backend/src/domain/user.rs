//! User data model.

use std::fmt;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Validation errors returned by [`User::try_new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserValidationError {
    EmptyId,
    InvalidId,
    EmptyDisplayName,
}

impl fmt::Display for UserValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => write!(f, "user id must not be empty"),
            Self::InvalidId => write!(f, "user id must be a valid UUID"),
            Self::EmptyDisplayName => write!(f, "display name must not be empty"),
        }
    }
}

impl std::error::Error for UserValidationError {}

/// Stable user identifier stored as a UUID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct UserId(Uuid, String);

impl UserId {
    /// Validate and construct a [`UserId`] from borrowed input.
    pub fn new(id: impl AsRef<str>) -> Result<Self, UserValidationError> {
        Self::from_owned(id.as_ref().to_owned())
    }

    fn from_owned(id: String) -> Result<Self, UserValidationError> {
        if id.is_empty() {
            return Err(UserValidationError::EmptyId);
        }
        if id.trim() != id {
            return Err(UserValidationError::InvalidId);
        }

        let parsed = Uuid::parse_str(&id).map_err(|_| UserValidationError::InvalidId)?;
        Ok(Self(parsed, id))
    }

    /// Access the underlying UUID.
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl AsRef<str> for UserId {
    fn as_ref(&self) -> &str {
        self.1.as_str()
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_ref())
    }
}

impl From<UserId> for String {
    fn from(value: UserId) -> Self {
        let UserId(_, raw) = value;
        raw
    }
}

impl TryFrom<String> for UserId {
    type Error = UserValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_owned(value)
    }
}

/// Human readable display name for the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DisplayName(String);

impl DisplayName {
    /// Validate and construct a [`DisplayName`] from owned input.
    pub fn new(display_name: impl Into<String>) -> Result<Self, UserValidationError> {
        Self::from_owned(display_name.into())
    }

    fn from_owned(display_name: String) -> Result<Self, UserValidationError> {
        if display_name.trim().is_empty() {
            return Err(UserValidationError::EmptyDisplayName);
        }

        Ok(Self(display_name))
    }
}

impl AsRef<str> for DisplayName {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for DisplayName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_ref())
    }
}

impl From<DisplayName> for String {
    fn from(value: DisplayName) -> Self {
        value.0
    }
}

impl TryFrom<String> for DisplayName {
    type Error = UserValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_owned(value)
    }
}

/// Application user.
///
/// ## Invariants
/// - `id` must be a valid UUID string.
/// - `display_name` must be non-empty once trimmed of whitespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
#[serde(try_from = "UserDto", into = "UserDto")]
pub struct User {
    #[schema(value_type = String, example = "3fa85f64-5717-4562-b3fc-2c963f66afa6")]
    id: UserId,
    #[schema(value_type = String, example = "Ada Lovelace")]
    #[serde(alias = "display_name")]
    display_name: DisplayName,
}

impl User {
    /// Build a new [`User`], panicking if validation fails.
    pub fn new(id: impl AsRef<str>, display_name: impl Into<String>) -> Self {
        match Self::try_new(id, display_name) {
            Ok(value) => value,
            Err(err) => panic!("user values must satisfy validation: {err}"),
        }
    }

    /// Fallible constructor enforcing identifier and display name invariants.
    pub fn try_new(
        id: impl AsRef<str>,
        display_name: impl Into<String>,
    ) -> Result<Self, UserValidationError> {
        let id = UserId::new(id)?;
        let display_name = DisplayName::new(display_name)?;

        Ok(Self { id, display_name })
    }

    /// Stable user identifier.
    pub fn id(&self) -> &UserId {
        &self.id
    }

    /// Display name shown to other users.
    pub fn display_name(&self) -> &DisplayName {
        &self.display_name
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct UserDto {
    id: String,
    #[serde(alias = "display_name")]
    display_name: String,
}

impl From<User> for UserDto {
    fn from(value: User) -> Self {
        let User { id, display_name } = value;
        Self {
            id: id.to_string(),
            display_name: display_name.into(),
        }
    }
}

impl TryFrom<UserDto> for User {
    type Error = UserValidationError;

    fn try_from(value: UserDto) -> Result<Self, Self::Error> {
        User::try_new(value.id, value.display_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};
    use rstest_bdd::{given, then, when};
    use serde_json::json;

    const VALID_ID: &str = "3fa85f64-5717-4562-b3fc-2c963f66afa6";

    #[fixture]
    fn valid_id() -> &'static str {
        VALID_ID
    }

    #[fixture]
    fn valid_display_name() -> &'static str {
        "Ada Lovelace"
    }

    #[rstest]
    fn new_panics_when_invalid_id() {
        let result = std::panic::catch_unwind(|| User::new("", "Ada"));
        assert!(result.is_err());
    }

    #[rstest]
    fn try_new_rejects_invalid_uuid(valid_display_name: &str) {
        let result = User::try_new("not-a-uuid", valid_display_name);
        assert!(matches!(result, Err(UserValidationError::InvalidId)));
    }

    #[rstest]
    fn try_new_rejects_uuid_with_whitespace(valid_display_name: &str) {
        let id = format!(" {VALID_ID} ");
        let result = User::try_new(id, valid_display_name);
        assert!(matches!(result, Err(UserValidationError::InvalidId)));
    }

    #[rstest]
    fn try_new_rejects_empty_display_name(valid_id: &str) {
        let result = User::try_new(valid_id, "   ");
        assert!(matches!(result, Err(UserValidationError::EmptyDisplayName)));
    }

    #[rstest]
    fn try_new_accepts_valid_inputs(valid_id: &str, valid_display_name: &str) {
        let user = User::try_new(valid_id, valid_display_name).expect("valid inputs");
        assert_eq!(user.id().as_ref(), valid_id);
        assert_eq!(user.display_name().as_ref(), valid_display_name);
    }

    #[rstest]
    fn serde_round_trips_alias(valid_id: &str, valid_display_name: &str) {
        let camel = json!({"id": valid_id, "displayName": valid_display_name});
        let snake = json!({"id": valid_id, "display_name": valid_display_name});
        let from_camel: User = serde_json::from_value(camel).expect("camelCase");
        let from_snake: User = serde_json::from_value(snake).expect("snake_case");
        assert_eq!(from_camel, from_snake);

        let value = serde_json::to_value(from_snake).expect("serialise to JSON");
        assert_eq!(
            value.get("displayName").and_then(|v| v.as_str()),
            Some(valid_display_name)
        );
        assert!(value.get("display_name").is_none());
    }

    #[given("a valid user payload")]
    fn a_valid_user_payload(valid_id: &str, valid_display_name: &str) -> (String, String) {
        (valid_id.to_owned(), valid_display_name.to_owned())
    }

    #[when("the user is constructed")]
    fn the_user_is_constructed(payload: (String, String)) -> Result<User, UserValidationError> {
        User::try_new(payload.0, payload.1)
    }

    #[then("the user is returned")]
    fn the_user_is_returned(result: Result<User, UserValidationError>, valid_id: &str) {
        let user = result.expect("user should be created");
        assert_eq!(user.id().as_ref(), valid_id);
    }

    #[rstest]
    fn constructing_a_user_happy_path(valid_display_name: &str, valid_id: &str) {
        let payload = a_valid_user_payload(valid_id, valid_display_name);
        let result = the_user_is_constructed(payload);
        the_user_is_returned(result, valid_id);
    }

    #[given("a payload with an empty display name")]
    fn a_payload_with_an_empty_display_name(valid_id: &str) -> (String, String) {
        (valid_id.to_owned(), "   ".to_owned())
    }

    #[then("user construction fails")]
    fn user_construction_fails(result: Result<User, UserValidationError>) {
        assert!(matches!(result, Err(UserValidationError::EmptyDisplayName)));
    }

    #[rstest]
    fn constructing_a_user_unhappy_path(valid_id: &str) {
        let payload = a_payload_with_an_empty_display_name(valid_id);
        let result = the_user_is_constructed(payload);
        user_construction_fails(result);
    }
}
