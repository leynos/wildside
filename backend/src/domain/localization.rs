//! Localisation primitives shared by catalogue and descriptor domain types.
//!
//! The backend persists localised copy as JSON, but the domain represents it
//! as typed maps so callers can validate structure before persistence.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Locale code key used in localisation maps (for example `en-GB`).
pub type LocaleCode = String;

/// Localised string set for one locale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct LocalizedStringSet {
    pub name: String,
    pub short_label: Option<String>,
    pub description: Option<String>,
}

impl LocalizedStringSet {
    /// Create a new localized string set.
    pub fn new(
        name: impl Into<String>,
        short_label: Option<String>,
        description: Option<String>,
    ) -> Self {
        Self {
            name: name.into(),
            short_label,
            description,
        }
    }
}

/// Validation errors returned by [`LocalizationMap::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalizationValidationError {
    EmptyMap,
    InvalidLocaleCode { locale: String },
    EmptyName { locale: String },
}

impl fmt::Display for LocalizationValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyMap => write!(f, "localizations must contain at least one locale"),
            Self::InvalidLocaleCode { locale } => {
                write!(f, "locale code '{locale}' must not be empty or padded")
            }
            Self::EmptyName { locale } => {
                write!(f, "localized name for locale '{locale}' must not be empty")
            }
        }
    }
}

impl std::error::Error for LocalizationValidationError {}

/// Localisation map keyed by locale code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LocalizationMap(BTreeMap<LocaleCode, LocalizedStringSet>);

impl LocalizationMap {
    /// Validate and create a localisation map.
    pub fn new(
        values: BTreeMap<LocaleCode, LocalizedStringSet>,
    ) -> Result<Self, LocalizationValidationError> {
        if values.is_empty() {
            return Err(LocalizationValidationError::EmptyMap);
        }

        for (locale, set) in &values {
            if locale.trim().is_empty() || locale.trim() != locale {
                return Err(LocalizationValidationError::InvalidLocaleCode {
                    locale: locale.clone(),
                });
            }
            if set.name.trim().is_empty() {
                return Err(LocalizationValidationError::EmptyName {
                    locale: locale.clone(),
                });
            }
        }

        Ok(Self(values))
    }

    /// Borrow the underlying localisation map.
    pub fn as_map(&self) -> &BTreeMap<LocaleCode, LocalizedStringSet> {
        &self.0
    }
}

impl TryFrom<BTreeMap<LocaleCode, LocalizedStringSet>> for LocalizationMap {
    type Error = LocalizationValidationError;

    fn try_from(value: BTreeMap<LocaleCode, LocalizedStringSet>) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for localisation map validation.

    use super::*;
    use rstest::rstest;

    #[rstest]
    fn localization_map_accepts_valid_values() {
        let mut values = BTreeMap::new();
        values.insert(
            "en-GB".to_owned(),
            LocalizedStringSet::new("Nature walk", None, Some("Scenic route".to_owned())),
        );

        let map = LocalizationMap::new(values).expect("valid localizations");
        assert_eq!(map.as_map().len(), 1);
    }

    #[rstest]
    fn localization_map_rejects_empty_map() {
        let err = LocalizationMap::new(BTreeMap::new()).expect_err("empty map should fail");
        assert_eq!(err, LocalizationValidationError::EmptyMap);
    }

    #[rstest]
    fn localization_map_rejects_invalid_locale_code() {
        let mut values = BTreeMap::new();
        values.insert(
            " en-GB ".to_owned(),
            LocalizedStringSet::new("Nature walk", None, None),
        );

        let err = LocalizationMap::new(values).expect_err("padded locale should fail");
        assert!(matches!(
            err,
            LocalizationValidationError::InvalidLocaleCode { .. }
        ));
    }

    #[rstest]
    fn localization_map_rejects_empty_name() {
        let mut values = BTreeMap::new();
        values.insert(
            "en-GB".to_owned(),
            LocalizedStringSet::new("   ", None, None),
        );

        let err = LocalizationMap::new(values).expect_err("empty localized name should fail");
        assert!(matches!(err, LocalizationValidationError::EmptyName { .. }));
    }
}
