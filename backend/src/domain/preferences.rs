//! User preferences and related domain types.
//!
//! This module defines the `UserPreferences` aggregate, which captures a user's
//! selected interest themes, safety toggles, and display unit system. Preferences
//! support optimistic concurrency via revision numbers.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::UserId;

/// The unit system for distance and elevation display.
///
/// # Examples
///
/// ```
/// # use backend::domain::UnitSystem;
/// let metric = UnitSystem::Metric;
/// let imperial = UnitSystem::Imperial;
///
/// assert_ne!(metric, imperial);
/// assert_eq!(UnitSystem::default(), UnitSystem::Metric);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UnitSystem {
    /// Metric units (kilometres, metres).
    #[default]
    Metric,
    /// Imperial units (miles, feet).
    Imperial,
}

impl UnitSystem {
    /// Returns the database string representation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use backend::domain::UnitSystem;
    /// assert_eq!(UnitSystem::Metric.as_str(), "metric");
    /// assert_eq!(UnitSystem::Imperial.as_str(), "imperial");
    /// ```
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Metric => "metric",
            Self::Imperial => "imperial",
        }
    }
}

impl std::fmt::Display for UnitSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error returned when parsing an unknown unit system string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseUnitSystemError {
    /// The unrecognised input value.
    pub input: String,
}

impl std::fmt::Display for ParseUnitSystemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown unit system: {}", self.input)
    }
}

impl std::error::Error for ParseUnitSystemError {}

impl std::str::FromStr for UnitSystem {
    type Err = ParseUnitSystemError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "metric" => Ok(Self::Metric),
            "imperial" => Ok(Self::Imperial),
            _ => Err(ParseUnitSystemError {
                input: s.to_owned(),
            }),
        }
    }
}

/// User preferences for interests, safety settings, and display options.
///
/// Preferences use optimistic concurrency via the `revision` field. Clients
/// must provide the current revision when updating; mismatches result in
/// conflict errors.
///
/// # Examples
///
/// ```
/// # use backend::domain::{UserId, UserPreferences, UnitSystem};
/// # use chrono::Utc;
/// let prefs = UserPreferences {
///     user_id: UserId::random(),
///     interest_theme_ids: vec![],
///     safety_toggle_ids: vec![],
///     unit_system: UnitSystem::Metric,
///     revision: 1,
///     updated_at: Utc::now(),
/// };
///
/// assert_eq!(prefs.revision, 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct UserPreferences {
    /// The user these preferences belong to.
    pub user_id: UserId,
    /// Selected interest theme IDs.
    pub interest_theme_ids: Vec<Uuid>,
    /// Enabled safety toggle IDs.
    pub safety_toggle_ids: Vec<Uuid>,
    /// Display unit system.
    pub unit_system: UnitSystem,
    /// Revision number for optimistic concurrency.
    pub revision: u32,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

impl UserPreferences {
    /// Create a new preferences instance with default values.
    ///
    /// Initialises with empty theme and safety selections, metric units,
    /// revision 1, and the current timestamp.
    pub fn new_default(user_id: UserId) -> Self {
        UserPreferencesBuilder::new(user_id).build()
    }

    /// Create a builder for constructing preferences incrementally.
    pub fn builder(user_id: UserId) -> UserPreferencesBuilder {
        UserPreferencesBuilder::new(user_id)
    }
}

/// Builder for constructing [`UserPreferences`] incrementally.
#[derive(Debug, Clone)]
pub struct UserPreferencesBuilder {
    user_id: UserId,
    interest_theme_ids: Vec<Uuid>,
    safety_toggle_ids: Vec<Uuid>,
    unit_system: UnitSystem,
    revision: u32,
    updated_at: Option<DateTime<Utc>>,
}

impl UserPreferencesBuilder {
    /// Create a new builder for the given user.
    pub fn new(user_id: UserId) -> Self {
        Self {
            user_id,
            interest_theme_ids: Vec::new(),
            safety_toggle_ids: Vec::new(),
            unit_system: UnitSystem::default(),
            revision: 1,
            updated_at: None,
        }
    }

    /// Set the interest theme IDs.
    pub fn interest_theme_ids(mut self, ids: Vec<Uuid>) -> Self {
        self.interest_theme_ids = ids;
        self
    }

    /// Set the safety toggle IDs.
    pub fn safety_toggle_ids(mut self, ids: Vec<Uuid>) -> Self {
        self.safety_toggle_ids = ids;
        self
    }

    /// Set the unit system.
    pub fn unit_system(mut self, system: UnitSystem) -> Self {
        self.unit_system = system;
        self
    }

    /// Set the revision number.
    pub fn revision(mut self, rev: u32) -> Self {
        self.revision = rev;
        self
    }

    /// Set the updated timestamp.
    pub fn updated_at(mut self, ts: DateTime<Utc>) -> Self {
        self.updated_at = Some(ts);
        self
    }

    /// Build the final [`UserPreferences`] instance.
    pub fn build(self) -> UserPreferences {
        UserPreferences {
            user_id: self.user_id,
            interest_theme_ids: self.interest_theme_ids,
            safety_toggle_ids: self.safety_toggle_ids,
            unit_system: self.unit_system,
            revision: self.revision,
            updated_at: self.updated_at.unwrap_or_else(Utc::now),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for this module.
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn unit_system_default_is_metric() {
        assert_eq!(UnitSystem::default(), UnitSystem::Metric);
    }

    #[rstest]
    #[case::metric("metric", UnitSystem::Metric)]
    #[case::imperial("imperial", UnitSystem::Imperial)]
    fn unit_system_parses_valid_strings(#[case] input: &str, #[case] expected: UnitSystem) {
        let parsed: UnitSystem = input.parse().expect("valid unit system");
        assert_eq!(parsed, expected);
    }

    #[rstest]
    #[case::unknown("unknown")]
    #[case::empty("")]
    #[case::capitalised("Metric")]
    fn unit_system_rejects_invalid_strings(#[case] input: &str) {
        let result: Result<UnitSystem, _> = input.parse();
        assert!(result.is_err());
    }

    #[rstest]
    fn unit_system_as_str_matches_parse() {
        for unit in [UnitSystem::Metric, UnitSystem::Imperial] {
            let s = unit.as_str();
            let parsed: UnitSystem = s.parse().expect("round-trip should succeed");
            assert_eq!(parsed, unit);
        }
    }

    #[rstest]
    fn unit_system_serde_roundtrip() {
        for unit in [UnitSystem::Metric, UnitSystem::Imperial] {
            let json = serde_json::to_string(&unit).expect("serialise");
            let parsed: UnitSystem = serde_json::from_str(&json).expect("deserialise");
            assert_eq!(parsed, unit);
        }
    }

    #[rstest]
    fn user_preferences_new_default() {
        let user_id = UserId::random();
        let prefs = UserPreferences::new_default(user_id.clone());

        assert_eq!(prefs.user_id, user_id);
        assert!(prefs.interest_theme_ids.is_empty());
        assert!(prefs.safety_toggle_ids.is_empty());
        assert_eq!(prefs.unit_system, UnitSystem::Metric);
        assert_eq!(prefs.revision, 1);
    }

    #[rstest]
    fn user_preferences_builder() {
        let user_id = UserId::random();
        let theme_id = Uuid::new_v4();
        let safety_id = Uuid::new_v4();

        let prefs = UserPreferences::builder(user_id.clone())
            .interest_theme_ids(vec![theme_id])
            .safety_toggle_ids(vec![safety_id])
            .unit_system(UnitSystem::Imperial)
            .revision(3)
            .build();

        assert_eq!(prefs.user_id, user_id);
        assert_eq!(prefs.interest_theme_ids, vec![theme_id]);
        assert_eq!(prefs.safety_toggle_ids, vec![safety_id]);
        assert_eq!(prefs.unit_system, UnitSystem::Imperial);
        assert_eq!(prefs.revision, 3);
    }
}
