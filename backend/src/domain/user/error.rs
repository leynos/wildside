//! Validation errors for the user data model.

use std::fmt;

use super::{DISPLAY_NAME_MAX, DISPLAY_NAME_MIN};

/// Validation errors returned by [`super::User::try_from_strings`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserValidationError {
    /// The user identifier was empty.
    EmptyId,
    /// The user identifier was not a valid UUID, or had surrounding whitespace.
    InvalidId,
    /// The display name was empty once trimmed of whitespace.
    EmptyDisplayName,
    /// The display name had fewer characters than the minimum.
    DisplayNameTooShort {
        /// Minimum permitted length in characters.
        min: usize,
    },
    /// The display name had more characters than the maximum.
    DisplayNameTooLong {
        /// Maximum permitted length in characters.
        max: usize,
    },
    /// Display name had a character other than ASCII letters, digits, spaces or underscores.
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

impl UserValidationError {
    /// Metadata for display name validation failures (code, message, field).
    pub fn display_name_error_meta(&self) -> Option<(&'static str, String, &'static str)> {
        match self {
            Self::EmptyDisplayName => Some((
                "empty",
                "Display name must not be empty.".to_owned(),
                "displayName",
            )),
            Self::DisplayNameTooShort { .. } => Some((
                "too_short",
                format!("Display name must be at least {DISPLAY_NAME_MIN} characters."),
                "displayName",
            )),
            Self::DisplayNameTooLong { .. } => Some((
                "too_long",
                format!("Display name must be at most {DISPLAY_NAME_MAX} characters."),
                "displayName",
            )),
            Self::DisplayNameInvalidCharacters => Some((
                "invalid_chars",
                "Only alphanumeric characters, spaces, and underscores are allowed.".to_owned(),
                "displayName",
            )),
            _ => None,
        }
    }
}
