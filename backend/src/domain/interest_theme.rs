//! Interest theme identifiers.
//!
//! Purpose: represent interest themes as validated UUIDs so adapters and
//! services share a stable identifier type.

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Validation errors returned by [`InterestThemeId::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterestThemeIdValidationError {
    EmptyId,
    InvalidId,
}

impl fmt::Display for InterestThemeIdValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => write!(f, "interest theme id must not be empty"),
            Self::InvalidId => write!(f, "interest theme id must be a valid UUID"),
        }
    }
}

impl std::error::Error for InterestThemeIdValidationError {}

/// Stable interest theme identifier stored as a UUID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct InterestThemeId(Uuid, String);

impl InterestThemeId {
    /// Validate and construct an [`InterestThemeId`] from borrowed input.
    pub fn new(id: impl AsRef<str>) -> Result<Self, InterestThemeIdValidationError> {
        let raw = id.as_ref();
        if raw.is_empty() {
            return Err(InterestThemeIdValidationError::EmptyId);
        }
        if raw.trim() != raw {
            return Err(InterestThemeIdValidationError::InvalidId);
        }

        let parsed = Uuid::parse_str(raw).map_err(|_| InterestThemeIdValidationError::InvalidId)?;
        Ok(Self(parsed, raw.to_owned()))
    }

    /// Construct an [`InterestThemeId`] directly from a UUID.
    pub fn from_uuid(uuid: Uuid) -> Self {
        let raw = uuid.to_string();
        Self(uuid, raw)
    }

    /// Access the underlying UUID.
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl AsRef<str> for InterestThemeId {
    fn as_ref(&self) -> &str {
        self.1.as_str()
    }
}

impl fmt::Display for InterestThemeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_ref())
    }
}

impl From<InterestThemeId> for String {
    fn from(value: InterestThemeId) -> Self {
        let InterestThemeId(_, raw) = value;
        raw
    }
}

impl TryFrom<String> for InterestThemeId {
    type Error = InterestThemeIdValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err(InterestThemeIdValidationError::EmptyId);
        }
        if value.trim() != value {
            return Err(InterestThemeIdValidationError::InvalidId);
        }

        let parsed =
            Uuid::parse_str(&value).map_err(|_| InterestThemeIdValidationError::InvalidId)?;
        Ok(Self(parsed, value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("3fa85f64-5717-4562-b3fc-2c963f66afa6", true)]
    #[case("", false)]
    #[case("not-a-uuid", false)]
    #[case(" 3fa85f64-5717-4562-b3fc-2c963f66afa6", false)]
    fn interest_theme_id_parsing(#[case] input: &str, #[case] should_succeed: bool) {
        let result = InterestThemeId::new(input);
        assert_eq!(result.is_ok(), should_succeed);
    }
}
