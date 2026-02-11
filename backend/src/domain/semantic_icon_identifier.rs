//! Semantic icon identifier used by catalogue and descriptor types.
//!
//! The identifier is a semantic key (for example `category:nature`) that the
//! client resolves to presentation details.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Validation errors returned by [`SemanticIconIdentifier::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticIconIdentifierValidationError {
    Empty,
    InvalidFormat,
}

impl fmt::Display for SemanticIconIdentifierValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "semantic icon identifier must not be empty"),
            Self::InvalidFormat => write!(
                f,
                "semantic icon identifier must use '<namespace>:<name>' with lowercase ASCII, digits, '-' or '_'"
            ),
        }
    }
}

impl std::error::Error for SemanticIconIdentifierValidationError {}

/// Semantic icon identifier shared across read-model entities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SemanticIconIdentifier(String);

impl SemanticIconIdentifier {
    /// Validate and construct a semantic icon identifier.
    pub fn new(value: impl AsRef<str>) -> Result<Self, SemanticIconIdentifierValidationError> {
        let value = value.as_ref();
        if value.trim().is_empty() {
            return Err(SemanticIconIdentifierValidationError::Empty);
        }
        if value.trim() != value || !is_valid_icon_identifier(value) {
            return Err(SemanticIconIdentifierValidationError::InvalidFormat);
        }

        Ok(Self(value.to_owned()))
    }
}

impl AsRef<str> for SemanticIconIdentifier {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for SemanticIconIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_ref())
    }
}

impl From<SemanticIconIdentifier> for String {
    fn from(value: SemanticIconIdentifier) -> Self {
        value.0
    }
}

impl TryFrom<String> for SemanticIconIdentifier {
    type Error = SemanticIconIdentifierValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

fn is_valid_icon_identifier(value: &str) -> bool {
    let Some((namespace, name)) = value.split_once(':') else {
        return false;
    };
    if namespace.is_empty() || name.is_empty() {
        return false;
    }

    let valid_chars = |segment: &str| {
        segment
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
    };

    valid_chars(namespace) && valid_chars(name)
}

#[cfg(test)]
mod tests {
    //! Unit tests for semantic icon identifier validation.

    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("category:nature")]
    #[case("badge:family_friendly")]
    #[case("safety:wheelchair-access")]
    fn accepts_valid_identifiers(#[case] input: &str) {
        let icon = SemanticIconIdentifier::new(input).expect("valid icon id");
        assert_eq!(icon.as_ref(), input);
    }

    #[rstest]
    #[case("")]
    #[case("   ")]
    fn rejects_empty_identifiers(#[case] input: &str) {
        let err = SemanticIconIdentifier::new(input).expect_err("empty icon id should fail");
        assert_eq!(err, SemanticIconIdentifierValidationError::Empty);
    }

    #[rstest]
    #[case("category")]
    #[case(":nature")]
    #[case("category:")]
    #[case("Category:nature")]
    #[case("category:na ture")]
    #[case("category:nature ")]
    fn rejects_invalid_identifiers(#[case] input: &str) {
        let err = SemanticIconIdentifier::new(input).expect_err("invalid icon id should fail");
        assert_eq!(err, SemanticIconIdentifierValidationError::InvalidFormat);
    }
}
