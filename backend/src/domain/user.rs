//! User data model.

use std::fmt;

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use utoipa::ToSchema;
use uuid::Uuid;

/// Validation errors returned by [`User::try_from_strings`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserValidationError {
    EmptyId,
    InvalidId,
    EmptyDisplayName,
    DisplayNameTooShort { min: usize },
    DisplayNameTooLong { max: usize },
    DisplayNameInvalidCharacters,
}

impl fmt::Display for UserValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => write!(f, "user id must not be empty"),
            Self::InvalidId => write!(f, "user id must be a valid UUID"),
            Self::EmptyDisplayName => write!(f, "display name must not be empty"),
            Self::DisplayNameTooShort { min } => {
                write!(f, "display name must be at least {min} characters")
            }
            Self::DisplayNameTooLong { max } => {
                write!(f, "display name must be at most {max} characters")
            }
            Self::DisplayNameInvalidCharacters => write!(
                f,
                "display name may only contain letters, numbers, spaces, or underscores",
            ),
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

    /// Generate a new random [`UserId`].
    pub fn random() -> Self {
        let uuid = Uuid::new_v4();
        Self(uuid, uuid.to_string())
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

/// Minimum allowed length for a display name.
pub const DISPLAY_NAME_MIN: usize = 3;
/// Maximum allowed length for a display name.
pub const DISPLAY_NAME_MAX: usize = 32;

static DISPLAY_NAME_RE: OnceLock<Regex> = OnceLock::new();

fn display_name_regex() -> &'static Regex {
    DISPLAY_NAME_RE.get_or_init(|| {
        // Length is enforced separately; this regex constrains allowed characters.
        let pattern = "^[A-Za-z0-9_ ]+$";
        Regex::new(pattern)
            .unwrap_or_else(|error| panic!("display name regex failed to compile: {error}"))
    })
}

impl DisplayName {
    /// Validate and construct a [`DisplayName`] from owned input.
    pub fn new(display_name: impl Into<String>) -> Result<Self, UserValidationError> {
        Self::from_owned(display_name.into())
    }

    fn from_owned(display_name: String) -> Result<Self, UserValidationError> {
        if display_name.trim().is_empty() {
            return Err(UserValidationError::EmptyDisplayName);
        }

        let length = display_name.chars().count();
        if length < DISPLAY_NAME_MIN {
            return Err(UserValidationError::DisplayNameTooShort {
                min: DISPLAY_NAME_MIN,
            });
        }
        if length > DISPLAY_NAME_MAX {
            return Err(UserValidationError::DisplayNameTooLong {
                max: DISPLAY_NAME_MAX,
            });
        }

        if !display_name_regex().is_match(&display_name) {
            return Err(UserValidationError::DisplayNameInvalidCharacters);
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
    /// Build a new [`User`] from validated components.
    pub fn new(id: UserId, display_name: DisplayName) -> Self {
        Self { id, display_name }
    }

    /// Build a new [`User`] from string inputs, panicking if validation fails.
    ///
    /// Prefer [`User::new`] when components are already validated.
    pub fn from_strings(id: impl AsRef<str>, display_name: impl Into<String>) -> Self {
        match Self::try_from_strings(id, display_name) {
            Ok(value) => value,
            Err(err) => panic!("user string values must satisfy validation: {err}"),
        }
    }

    /// Fallible constructor enforcing identifier and display name invariants.
    ///
    /// Prefer [`User::new`] when components are already validated.
    pub fn try_from_strings(
        id: impl AsRef<str>,
        display_name: impl Into<String>,
    ) -> Result<Self, UserValidationError> {
        let id = UserId::new(id)?;
        let display_name = DisplayName::new(display_name)?;

        Ok(Self::new(id, display_name))
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
        User::try_from_strings(value.id, value.display_name)
    }
}

#[cfg(test)]
mod tests;
