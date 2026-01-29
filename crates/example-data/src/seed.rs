//! Generated user seed types.
//!
//! This module defines the output types from user generation. These types are
//! independent of backend domain types to avoid circular dependencies.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unit system preference for a generated user.
///
/// Mirrors the backend's `UnitSystem` enum without creating a dependency.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnitSystemSeed {
    /// Metric units (kilometres, metres, etc.).
    #[default]
    Metric,
    /// Imperial units (miles, feet, etc.).
    Imperial,
}

/// A generated example user record.
///
/// This type contains all the fields needed to create a user and their
/// preferences in the backend. It is designed to be converted into backend
/// domain types at the point of use.
///
/// # Example
///
/// ```
/// use example_data::{ExampleUserSeed, UnitSystemSeed};
/// use uuid::Uuid;
///
/// let user = ExampleUserSeed {
///     id: Uuid::new_v4(),
///     display_name: "Ada Lovelace".to_owned(),
///     interest_theme_ids: vec![Uuid::new_v4()],
///     safety_toggle_ids: vec![],
///     unit_system: UnitSystemSeed::Metric,
/// };
///
/// assert_eq!(user.display_name, "Ada Lovelace");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExampleUserSeed {
    /// Unique identifier for the user.
    pub id: Uuid,
    /// Human-readable display name.
    pub display_name: String,
    /// Selected interest theme identifiers.
    pub interest_theme_ids: Vec<Uuid>,
    /// Selected safety toggle identifiers.
    pub safety_toggle_ids: Vec<Uuid>,
    /// Unit system preference.
    pub unit_system: UnitSystemSeed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_system_seed_defaults_to_metric() {
        assert_eq!(UnitSystemSeed::default(), UnitSystemSeed::Metric);
    }

    #[test]
    fn unit_system_seed_serializes_lowercase() {
        let metric = serde_json::to_string(&UnitSystemSeed::Metric).expect("serialize");
        let imperial = serde_json::to_string(&UnitSystemSeed::Imperial).expect("serialize");
        assert_eq!(metric, "\"metric\"");
        assert_eq!(imperial, "\"imperial\"");
    }

    #[test]
    fn example_user_seed_serializes_to_camel_case() {
        let user = ExampleUserSeed {
            id: Uuid::nil(),
            display_name: "Test".to_owned(),
            interest_theme_ids: vec![],
            safety_toggle_ids: vec![],
            unit_system: UnitSystemSeed::Metric,
        };
        let json = serde_json::to_string(&user).expect("serialize");
        assert!(json.contains("displayName"));
        assert!(json.contains("interestThemeIds"));
        assert!(json.contains("safetyToggleIds"));
        assert!(json.contains("unitSystem"));
    }
}
