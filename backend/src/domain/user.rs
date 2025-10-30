//! User data model.

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

impl std::fmt::Display for UserValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyId => write!(f, "user id must not be empty"),
            Self::InvalidId => write!(f, "user id must be a valid UUID"),
            Self::EmptyDisplayName => write!(f, "display name must not be empty"),
        }
    }
}

impl std::error::Error for UserValidationError {}

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
    #[schema(example = "3fa85f64-5717-4562-b3fc-2c963f66afa6")]
    id: String,
    #[schema(example = "Ada Lovelace")]
    #[serde(alias = "display_name")]
    display_name: String,
}

impl User {
    /// Build a new [`User`], panicking if validation fails.
    pub fn new(id: impl Into<String>, display_name: impl Into<String>) -> Self {
        match Self::try_new(id, display_name) {
            Ok(value) => value,
            Err(err) => panic!("user values must satisfy validation: {err}"),
        }
    }

    /// Fallible constructor enforcing identifier and display name invariants.
    pub fn try_new(
        id: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Result<Self, UserValidationError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(UserValidationError::EmptyId);
        }
        Uuid::parse_str(id.trim()).map_err(|_| UserValidationError::InvalidId)?;

        let display_name = display_name.into();
        if display_name.trim().is_empty() {
            return Err(UserValidationError::EmptyDisplayName);
        }

        Ok(Self { id, display_name })
    }

    /// Stable user identifier.
    pub fn id(&self) -> &str {
        self.id.as_str()
    }

    /// Display name shown to other users.
    pub fn display_name(&self) -> &str {
        self.display_name.as_str()
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
        Self {
            id: value.id,
            display_name: value.display_name,
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
    fn try_new_rejects_empty_display_name(valid_id: &str) {
        let result = User::try_new(valid_id, "   ");
        assert!(matches!(result, Err(UserValidationError::EmptyDisplayName)));
    }

    #[rstest]
    fn try_new_accepts_valid_inputs(valid_id: &str, valid_display_name: &str) {
        let user = User::try_new(valid_id, valid_display_name).expect("valid inputs");
        assert_eq!(user.id(), valid_id);
        assert_eq!(user.display_name(), valid_display_name);
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
        assert_eq!(user.id(), valid_id);
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
