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

macro_rules! define_simple_descriptor {
    ($(#[$new_doc:meta])* $type_name:ident, $slug_field:literal, $type_doc:literal) => {
        #[doc = $type_doc]
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[serde(deny_unknown_fields)]
        pub struct $type_name {
            pub id: Uuid,
            pub slug: String,
            pub icon_key: SemanticIconIdentifier,
            pub localizations: LocalizationMap,
        }

        impl $type_name {
            $(#[$new_doc])*
            pub fn new(
                id: Uuid,
                slug: impl Into<String>,
                icon_key: SemanticIconIdentifier,
                localizations: LocalizationMap,
            ) -> Result<Self, DescriptorValidationError> {
                let slug = validate_slug(slug.into(), $slug_field)?;
                Ok(Self {
                    id,
                    slug,
                    icon_key,
                    localizations,
                })
            }
        }
    };
}

define_simple_descriptor!(
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
    Tag,
    "tag.slug",
    "Tag descriptor shown in route metadata."
);

define_simple_descriptor!(
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
    Badge,
    "badge.slug",
    "Badge descriptor shown in route summary metadata."
);

define_simple_descriptor!(
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
    SafetyToggle,
    "safety_toggle.slug",
    "Safety toggle descriptor used by user preferences."
);

/// Unvalidated payload used to construct a [`SafetyPreset`].
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
#[serde(try_from = "SafetyPresetDraft", into = "SafetyPresetDraft")]
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
    ///     LocalizedStringSet, LocalizationMap, SafetyPreset, SafetyPresetDraft,
    ///     SemanticIconIdentifier,
    /// };
    /// use uuid::Uuid;
    ///
    /// let toggle_id = Uuid::new_v4();
    /// let mut values = BTreeMap::new();
    /// values.insert(
    ///     "en-GB".to_owned(),
    ///     LocalizedStringSet::new("Night safe", None, None),
    /// );
    /// let draft = SafetyPresetDraft {
    ///     id: Uuid::new_v4(),
    ///     slug: "night-safe".to_owned(),
    ///     icon_key: SemanticIconIdentifier::new("preset:night-safe")
    ///         .expect("valid icon"),
    ///     localizations: LocalizationMap::new(values).expect("valid localizations"),
    ///     safety_toggle_ids: vec![toggle_id],
    /// };
    ///
    /// let preset = SafetyPreset::new(draft).expect("valid safety preset");
    /// assert_eq!(preset.slug, "night-safe");
    /// assert_eq!(preset.safety_toggle_ids, vec![toggle_id]);
    /// ```
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

impl TryFrom<SafetyPresetDraft> for SafetyPreset {
    type Error = DescriptorValidationError;

    fn try_from(value: SafetyPresetDraft) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<SafetyPreset> for SafetyPresetDraft {
    fn from(value: SafetyPreset) -> Self {
        Self {
            id: value.id,
            slug: value.slug,
            icon_key: value.icon_key,
            localizations: value.localizations,
            safety_toggle_ids: value.safety_toggle_ids,
        }
    }
}

fn validate_slug(value: String, field: &'static str) -> Result<String, DescriptorValidationError> {
    if has_whitespace_or_is_empty(&value) {
        return Err(DescriptorValidationError::InvalidSlug { field });
    }

    if !contains_only_valid_slug_chars(&value) {
        return Err(DescriptorValidationError::InvalidSlug { field });
    }

    Ok(value)
}

fn has_whitespace_or_is_empty(value: &str) -> bool {
    value.trim() != value || value.is_empty()
}

fn contains_only_valid_slug_chars(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
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
