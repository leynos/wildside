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
    /// Returned when the provided ID is empty.
    EmptyId,
    /// Returned when the ID is not a valid UUID or contains whitespace padding.
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
        let parsed = Self::validate_and_parse(raw)?;
        Ok(Self(parsed, raw.to_owned()))
    }

    /// Construct an [`InterestThemeId`] directly from a UUID.
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid, uuid.to_string())
    }

    /// Access the underlying UUID.
    #[rustfmt::skip]
    pub fn as_uuid(&self) -> &Uuid { &self.0 }

    fn validate_and_parse(id: &str) -> Result<Uuid, InterestThemeIdValidationError> {
        if id.is_empty() {
            return Err(InterestThemeIdValidationError::EmptyId);
        }
        if id.trim() != id {
            return Err(InterestThemeIdValidationError::InvalidId);
        }
        Uuid::parse_str(id).map_err(|_| InterestThemeIdValidationError::InvalidId)
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
        let parsed = Self::validate_and_parse(&value)?;
        Ok(Self(parsed, value))
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for this module.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("3fa85f64-5717-4562-b3fc-2c963f66afa6", true)]
    #[case("", false)]
    #[case("not-a-uuid", false)]
    #[case(" 3fa85f64-5717-4562-b3fc-2c963f66afa6", false)]
    #[case("3fa85f64-5717-4562-b3fc-2c963f66afa6 ", false)]
    fn interest_theme_id_parsing(#[case] input: &str, #[case] should_succeed: bool) {
        let result = InterestThemeId::new(input);
        assert_eq!(result.is_ok(), should_succeed);
    }
}
