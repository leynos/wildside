//! Descriptor read-model domain types.
//!
//! These descriptors are consumed by catalogue surfaces and user preference
//! workflows. They are domain-owned and validated before persistence.

use std::collections::HashSet;
use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::localization::{LocalizationMap, LocalizationValidationError};
use super::semantic_icon_identifier::{
    SemanticIconIdentifier, SemanticIconIdentifierValidationError,
};

#[cfg(test)]
mod tests;

/// Validation errors returned by descriptor constructors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescriptorValidationError {
    InvalidSlug { field: &'static str },
    EmptySafetyPresetToggleIds,
    DuplicateSafetyPresetToggleId { toggle_id: Uuid },
    Localization(LocalizationValidationError),
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

/// Tag descriptor shown in route metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct Tag {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: SemanticIconIdentifier,
    pub localizations: LocalizationMap,
}

impl Tag {
    /// Validate and construct a tag descriptor.
    pub fn new(
        id: Uuid,
        slug: impl Into<String>,
        icon_key: SemanticIconIdentifier,
        localizations: LocalizationMap,
    ) -> Result<Self, DescriptorValidationError> {
        let slug = validate_slug(slug.into(), "tag.slug")?;
        Ok(Self {
            id,
            slug,
            icon_key,
            localizations,
        })
    }
}

/// Badge descriptor shown in route summary metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct Badge {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: SemanticIconIdentifier,
    pub localizations: LocalizationMap,
}

impl Badge {
    /// Validate and construct a badge descriptor.
    pub fn new(
        id: Uuid,
        slug: impl Into<String>,
        icon_key: SemanticIconIdentifier,
        localizations: LocalizationMap,
    ) -> Result<Self, DescriptorValidationError> {
        let slug = validate_slug(slug.into(), "badge.slug")?;
        Ok(Self {
            id,
            slug,
            icon_key,
            localizations,
        })
    }
}

/// Safety toggle descriptor used by user preferences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct SafetyToggle {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: SemanticIconIdentifier,
    pub localizations: LocalizationMap,
}

impl SafetyToggle {
    /// Validate and construct a safety toggle descriptor.
    pub fn new(
        id: Uuid,
        slug: impl Into<String>,
        icon_key: SemanticIconIdentifier,
        localizations: LocalizationMap,
    ) -> Result<Self, DescriptorValidationError> {
        let slug = validate_slug(slug.into(), "safety_toggle.slug")?;
        Ok(Self {
            id,
            slug,
            icon_key,
            localizations,
        })
    }
}

/// Safety preset descriptor combining a validated toggle set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct SafetyPresetDraft {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: SemanticIconIdentifier,
    pub localizations: LocalizationMap,
    pub safety_toggle_ids: Vec<Uuid>,
}

/// Safety preset descriptor combining a validated toggle set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct SafetyPreset {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: SemanticIconIdentifier,
    pub localizations: LocalizationMap,
    pub safety_toggle_ids: Vec<Uuid>,
}

impl SafetyPreset {
    /// Validate and construct a safety preset descriptor.
    pub fn new(draft: SafetyPresetDraft) -> Result<Self, DescriptorValidationError> {
        let slug = validate_slug(draft.slug, "safety_preset.slug")?;
        ensure_non_empty_unique_toggle_ids(&draft.safety_toggle_ids)?;

        Ok(Self {
            id: draft.id,
            slug,
            icon_key: draft.icon_key,
            localizations: draft.localizations,
            safety_toggle_ids: draft.safety_toggle_ids,
        })
    }
}

fn validate_slug(value: String, field: &'static str) -> Result<String, DescriptorValidationError> {
    if value.trim() != value || value.is_empty() {
        return Err(DescriptorValidationError::InvalidSlug { field });
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
    {
        return Err(DescriptorValidationError::InvalidSlug { field });
    }

    Ok(value)
}

fn ensure_non_empty_unique_toggle_ids(ids: &[Uuid]) -> Result<(), DescriptorValidationError> {
    if ids.is_empty() {
        return Err(DescriptorValidationError::EmptySafetyPresetToggleIds);
    }

    let mut seen = HashSet::with_capacity(ids.len());
    for id in ids {
        if !seen.insert(*id) {
            return Err(DescriptorValidationError::DuplicateSafetyPresetToggleId {
                toggle_id: *id,
            });
        }
    }

    Ok(())
}
