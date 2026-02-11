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
use crate::domain::slug::is_valid_slug;

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
    /// Validate and construct the descriptor.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::collections::BTreeMap;
    ///
    /// use backend::domain::{
    ///     LocalizedStringSet, LocalizationMap, SemanticIconIdentifier, Tag,
    /// };
    /// use uuid::Uuid;
    ///
    /// let mut values = BTreeMap::new();
    /// values.insert(
    ///     "en-GB".to_owned(),
    ///     LocalizedStringSet::new("Family friendly", Some("Family".to_owned()), None),
    /// );
    /// let localizations = LocalizationMap::new(values).expect("valid localizations");
    /// let icon = SemanticIconIdentifier::new("tag:family").expect("valid icon");
    ///
    /// let tag = Tag::new(Uuid::new_v4(), "family-friendly", icon, localizations)
    ///     .expect("valid tag");
    /// assert_eq!(tag.slug, "family-friendly");
    /// ```
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
    /// Validate and construct the descriptor.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::collections::BTreeMap;
    ///
    /// use backend::domain::{
    ///     Badge, LocalizedStringSet, LocalizationMap, SemanticIconIdentifier,
    /// };
    /// use uuid::Uuid;
    ///
    /// let mut values = BTreeMap::new();
    /// values.insert(
    ///     "en-GB".to_owned(),
    ///     LocalizedStringSet::new("Accessible", None, Some("Step-free".to_owned())),
    /// );
    /// let localizations = LocalizationMap::new(values).expect("valid localizations");
    /// let icon = SemanticIconIdentifier::new("badge:accessible").expect("valid icon");
    ///
    /// let badge = Badge::new(Uuid::new_v4(), "accessible", icon, localizations)
    ///     .expect("valid badge");
    /// assert_eq!(badge.slug, "accessible");
    /// ```
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
    /// Validate and construct the descriptor.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::collections::BTreeMap;
    ///
    /// use backend::domain::{
    ///     LocalizedStringSet, LocalizationMap, SafetyToggle, SemanticIconIdentifier,
    /// };
    /// use uuid::Uuid;
    ///
    /// let mut values = BTreeMap::new();
    /// values.insert(
    ///     "en-GB".to_owned(),
    ///     LocalizedStringSet::new("Well lit", None, None),
    /// );
    /// let localizations = LocalizationMap::new(values).expect("valid localizations");
    /// let icon = SemanticIconIdentifier::new("safety:well-lit").expect("valid icon");
    ///
    /// let toggle = SafetyToggle::new(Uuid::new_v4(), "well-lit", icon, localizations)
    ///     .expect("valid safety toggle");
    /// assert_eq!(toggle.slug, "well-lit");
    /// ```
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

/// Interest theme descriptor consumed by user preference flows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct InterestTheme {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
}

impl InterestTheme {
    /// Construct an interest theme descriptor.
    #[rustfmt::skip]
    pub fn new(id: Uuid, name: impl Into<String>, description: Option<String>) -> Self {
        Self {
            id,
            name: name.into(),
            description,
        }
    }
}

/// Safety preset descriptor combining a validated toggle set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::collections::BTreeMap;
    ///
    /// use backend::domain::{
    ///     LocalizedStringSet, LocalizationMap, SafetyPreset, SemanticIconIdentifier,
    /// };
    /// use uuid::Uuid;
    ///
    /// let toggle_id = Uuid::new_v4();
    /// let mut values = BTreeMap::new();
    /// values.insert(
    ///     "en-GB".to_owned(),
    ///     LocalizedStringSet::new("Night safe", None, None),
    /// );
    /// let preset = SafetyPreset::new(SafetyPreset {
    ///     id: Uuid::new_v4(),
    ///     slug: "night-safe".to_owned(),
    ///     icon_key: SemanticIconIdentifier::new("preset:night-safe").expect("valid icon"),
    ///     localizations: LocalizationMap::new(values).expect("valid localizations"),
    ///     safety_toggle_ids: vec![toggle_id],
    /// })
    /// .expect("valid safety preset");
    /// assert_eq!(preset.slug, "night-safe");
    /// assert_eq!(preset.safety_toggle_ids, vec![toggle_id]);
    /// ```
    pub fn new(value: Self) -> Result<Self, DescriptorValidationError> {
        let Self {
            id,
            slug,
            icon_key,
            localizations,
            safety_toggle_ids,
        } = value;

        let slug = validate_slug(slug, "safety_preset.slug")?;
        ensure_non_empty_unique_toggle_ids(&safety_toggle_ids)?;

        Ok(Self {
            id,
            slug,
            icon_key,
            localizations,
            safety_toggle_ids,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
struct SafetyPresetSerde {
    id: Uuid,
    slug: String,
    icon_key: SemanticIconIdentifier,
    localizations: LocalizationMap,
    safety_toggle_ids: Vec<Uuid>,
}

impl<'de> Deserialize<'de> for SafetyPreset {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = SafetyPresetSerde::deserialize(deserializer)?;
        Self::new(Self {
            id: value.id,
            slug: value.slug,
            icon_key: value.icon_key,
            localizations: value.localizations,
            safety_toggle_ids: value.safety_toggle_ids,
        })
        .map_err(serde::de::Error::custom)
    }
}

fn validate_slug(value: String, field: &'static str) -> Result<String, DescriptorValidationError> {
    if !is_valid_slug(value.as_str()) {
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
