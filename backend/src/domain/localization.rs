//! Localization primitives shared by catalogue and descriptor domain types.
//!
//! The backend persists localized copy as JSON, but the domain represents it
//! as typed maps so callers can validate structure before persistence.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Locale code key used in localization maps (for example `en-GB`).
pub type LocaleCode = String;

/// Localized string set for one locale.
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
    ///
    /// # Examples
    ///
    /// ```rust
    /// use backend::domain::LocalizedStringSet;
    ///
    /// let set = LocalizedStringSet::new(
    ///     "Scenic route",
    ///     Some("Scenic".to_owned()),
    ///     Some("Coastal and cliff paths".to_owned()),
    /// );
    ///
    /// assert_eq!(set.name, "Scenic route");
    /// assert_eq!(set.short_label.as_deref(), Some("Scenic"));
    /// ```
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

/// Localization map keyed by locale code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    try_from = "BTreeMap<LocaleCode, LocalizedStringSet>",
    into = "BTreeMap<LocaleCode, LocalizedStringSet>"
)]
pub struct LocalizationMap(BTreeMap<LocaleCode, LocalizedStringSet>);

impl LocalizationMap {
    /// Validate and create a localization map.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::collections::BTreeMap;
    ///
    /// use backend::domain::{LocalizationMap, LocalizedStringSet};
    ///
    /// let mut values = BTreeMap::new();
    /// values.insert(
    ///     "en-GB".to_owned(),
    ///     LocalizedStringSet::new("Scenic route", Some("Scenic".to_owned()), None),
    /// );
    ///
    /// let map = LocalizationMap::new(values).expect("valid localization map");
    /// assert_eq!(map.as_map().len(), 1);
    /// ```
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

    /// Borrow the underlying localization map.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::collections::BTreeMap;
    ///
    /// use backend::domain::{LocalizationMap, LocalizedStringSet};
    ///
    /// let mut values = BTreeMap::new();
    /// values.insert(
    ///     "en-GB".to_owned(),
    ///     LocalizedStringSet::new("Nature walk", None, None),
    /// );
    /// let map = LocalizationMap::new(values).expect("valid localization map");
    ///
    /// assert!(map.as_map().contains_key("en-GB"));
    /// ```
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

impl From<LocalizationMap> for BTreeMap<LocaleCode, LocalizedStringSet> {
    fn from(value: LocalizationMap) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for localization map validation.

    use super::*;
    use rstest::rstest;
    use serde_json::json;

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

    #[rstest]
    fn localization_map_deserialization_enforces_validation() {
        let payload = json!({
            " en-GB ": {
                "name": "Scenic route"
            }
        });

        let err = serde_json::from_value::<LocalizationMap>(payload)
            .expect_err("invalid locale should fail deserialization");
        assert!(err.to_string().contains("must not be empty or padded"));
    }
}
