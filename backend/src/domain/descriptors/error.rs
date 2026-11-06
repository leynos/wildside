//! Validation errors returned by descriptor constructors.

use std::fmt;

use uuid::Uuid;

use crate::domain::localization::LocalizationValidationError;
use crate::domain::semantic_icon_identifier::SemanticIconIdentifierValidationError;

/// Validation errors returned by descriptor constructors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescriptorValidationError {
    /// A slug failed the lowercase ASCII slug rule.
    InvalidSlug {
        /// Dotted name of the slug field that failed, such as `tag.slug`.
        field: &'static str,
    },
    /// A safety preset was constructed with no toggle identifiers.
    EmptySafetyPresetToggleIds,
    /// A safety preset listed the same toggle identifier more than once.
    DuplicateSafetyPresetToggleId {
        /// The toggle identifier that appeared more than once.
        toggle_id: Uuid,
    },
    /// A localization map failed validation.
    Localization(LocalizationValidationError),
    /// A semantic icon identifier failed validation.
    IconIdentifier(SemanticIconIdentifierValidationError),
}

impl fmt::Display for DescriptorValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSlug { field } => write!(
                f,
                "{field} must contain lowercase ASCII letters, digits, and hyphens"
            ),
            Self::EmptySafetyPresetToggleIds => {
                write!(f, "safety_preset.safety_toggle_ids must not be empty")
            }
            Self::DuplicateSafetyPresetToggleId { toggle_id } => write!(
                f,
                "safety_preset.safety_toggle_ids contains duplicate id {toggle_id}"
            ),
            Self::Localization(error) => error.fmt(f),
            Self::IconIdentifier(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for DescriptorValidationError {}

impl From<LocalizationValidationError> for DescriptorValidationError {
    fn from(value: LocalizationValidationError) -> Self {
        Self::Localization(value)
    }
}

impl From<SemanticIconIdentifierValidationError> for DescriptorValidationError {
    fn from(value: SemanticIconIdentifierValidationError) -> Self {
        Self::IconIdentifier(value)
    }
}
