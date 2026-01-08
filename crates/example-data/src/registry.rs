//! Seed registry types and JSON parsing.
//!
//! This module defines the seed registry structure that holds named seed
//! definitions and descriptor IDs. The registry is loaded from JSON and
//! provides deterministic seed lookups.

use std::fs;
use std::path::Path;

use serde::Deserialize;
use uuid::Uuid;

use crate::error::RegistryError;

/// Current supported registry version.
const SUPPORTED_VERSION: u32 = 1;

/// A seed registry containing named seeds and descriptor IDs.
///
/// The registry is loaded from a JSON file and provides access to seed
/// definitions and the descriptor IDs that generated users can reference.
///
/// # Example
///
/// ```
/// use example_data::SeedRegistry;
///
/// let json = r#"{
///     "version": 1,
///     "interestThemeIds": ["3fa85f64-5717-4562-b3fc-2c963f66afa6"],
///     "safetyToggleIds": [],
///     "seeds": [{"name": "test", "seed": 42, "userCount": 5}]
/// }"#;
///
/// let registry = SeedRegistry::from_json(json).expect("valid registry");
/// assert_eq!(registry.seeds().len(), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedRegistry {
    version: u32,
    interest_theme_ids: Vec<Uuid>,
    safety_toggle_ids: Vec<Uuid>,
    seeds: Vec<SeedDefinition>,
}

impl SeedRegistry {
    /// Parses a seed registry from a JSON string.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError`] if:
    /// - The JSON is malformed
    /// - Required fields are missing
    /// - The version is unsupported
    /// - Any UUID is invalid
    /// - The seeds array is empty
    pub fn from_json(json: &str) -> Result<Self, RegistryError> {
        let raw: RawSeedRegistry =
            serde_json::from_str(json).map_err(|e| RegistryError::ParseError {
                message: e.to_string(),
            })?;

        Self::from_raw(raw)
    }

    /// Loads a seed registry from a JSON file.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError`] if the file cannot be read or parsed.
    pub fn from_file(path: &Path) -> Result<Self, RegistryError> {
        let contents = fs::read_to_string(path).map_err(|e| RegistryError::IoError {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;

        Self::from_json(&contents)
    }

    fn from_raw(raw: RawSeedRegistry) -> Result<Self, RegistryError> {
        // Validate version
        if raw.version != SUPPORTED_VERSION {
            return Err(RegistryError::UnsupportedVersion {
                expected: SUPPORTED_VERSION,
                actual: raw.version,
            });
        }

        // Validate and parse interest theme IDs
        let interest_theme_ids = raw
            .interest_theme_ids
            .into_iter()
            .enumerate()
            .map(|(index, id)| {
                Uuid::parse_str(&id)
                    .map_err(|_| RegistryError::InvalidInterestThemeId { index, value: id })
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Require at least one interest theme for user generation
        if interest_theme_ids.is_empty() {
            return Err(RegistryError::EmptyInterestThemes);
        }

        // Validate and parse safety toggle IDs
        let safety_toggle_ids = raw
            .safety_toggle_ids
            .into_iter()
            .enumerate()
            .map(|(index, id)| {
                Uuid::parse_str(&id)
                    .map_err(|_| RegistryError::InvalidSafetyToggleId { index, value: id })
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Validate seeds
        if raw.seeds.is_empty() {
            return Err(RegistryError::EmptySeeds);
        }

        let seeds = raw
            .seeds
            .into_iter()
            .map(|s| SeedDefinition {
                name: s.name,
                seed: s.seed,
                user_count: s.user_count,
            })
            .collect();

        Ok(Self {
            version: raw.version,
            interest_theme_ids,
            safety_toggle_ids,
            seeds,
        })
    }

    /// Returns the registry version.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Returns the available interest theme IDs.
    #[must_use]
    pub fn interest_theme_ids(&self) -> &[Uuid] {
        &self.interest_theme_ids
    }

    /// Returns the available safety toggle IDs.
    #[must_use]
    pub fn safety_toggle_ids(&self) -> &[Uuid] {
        &self.safety_toggle_ids
    }

    /// Returns all seed definitions.
    #[must_use]
    pub fn seeds(&self) -> &[SeedDefinition] {
        &self.seeds
    }

    /// Finds a seed definition by name.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::SeedNotFound`] if no seed with the given name
    /// exists.
    pub fn find_seed(&self, name: &str) -> Result<&SeedDefinition, RegistryError> {
        self.seeds
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| RegistryError::SeedNotFound {
                name: name.to_owned(),
            })
    }
}

/// A named seed definition for deterministic user generation.
///
/// Each seed has a unique name, an RNG seed value, and a user count that
/// determines how many users to generate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedDefinition {
    name: String,
    seed: u64,
    user_count: usize,
}

impl SeedDefinition {
    /// Returns the seed name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the RNG seed value.
    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    /// Returns the number of users to generate.
    #[must_use]
    pub const fn user_count(&self) -> usize {
        self.user_count
    }
}

/// Raw JSON representation for deserialization.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSeedRegistry {
    version: u32,
    interest_theme_ids: Vec<String>,
    safety_toggle_ids: Vec<String>,
    seeds: Vec<RawSeedDefinition>,
}

/// Raw JSON representation of a seed definition.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSeedDefinition {
    name: String,
    seed: u64,
    user_count: usize,
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    const VALID_JSON: &str = r#"{
        "version": 1,
        "interestThemeIds": [
            "3fa85f64-5717-4562-b3fc-2c963f66afa6",
            "4fa85f64-5717-4562-b3fc-2c963f66afa7"
        ],
        "safetyToggleIds": [
            "7fa85f64-5717-4562-b3fc-2c963f66afa6"
        ],
        "seeds": [
            {"name": "mossy-owl", "seed": 2026, "userCount": 12},
            {"name": "snowy-penguin", "seed": 1234, "userCount": 5}
        ]
    }"#;

    #[test]
    fn parses_valid_registry() {
        let registry = SeedRegistry::from_json(VALID_JSON).expect("valid registry");

        assert_eq!(registry.version(), 1);
        assert_eq!(registry.interest_theme_ids().len(), 2);
        assert_eq!(registry.safety_toggle_ids().len(), 1);
        assert_eq!(registry.seeds().len(), 2);
    }

    #[test]
    fn finds_seed_by_name() {
        let registry = SeedRegistry::from_json(VALID_JSON).expect("valid registry");
        let seed = registry.find_seed("mossy-owl").expect("seed found");

        assert_eq!(seed.name(), "mossy-owl");
        assert_eq!(seed.seed(), 2026);
        assert_eq!(seed.user_count(), 12);
    }

    #[test]
    fn returns_error_for_unknown_seed() {
        let registry = SeedRegistry::from_json(VALID_JSON).expect("valid registry");
        let result = registry.find_seed("unknown");

        assert_eq!(
            result,
            Err(RegistryError::SeedNotFound {
                name: "unknown".to_owned()
            })
        );
    }

    /// Tests that use pattern matching for parse errors (message content varies).
    #[rstest]
    #[case::malformed_json("not valid json")]
    #[case::missing_version(
        r#"{"interestThemeIds": [], "safetyToggleIds": [], "seeds": [{"name": "a", "seed": 1, "userCount": 1}]}"#
    )]
    fn rejects_json_with_parse_error(#[case] json: &str) {
        let result = SeedRegistry::from_json(json);
        assert!(matches!(result, Err(RegistryError::ParseError { .. })));
    }

    /// Tests that check exact error variants.
    #[rstest]
    #[case::unsupported_version(
        r#"{"version": 99, "interestThemeIds": [], "safetyToggleIds": [], "seeds": [{"name": "a", "seed": 1, "userCount": 1}]}"#,
        RegistryError::UnsupportedVersion { expected: 1, actual: 99 }
    )]
    #[case::invalid_interest_theme_uuid(
        r#"{"version": 1, "interestThemeIds": ["not-a-uuid"], "safetyToggleIds": [], "seeds": [{"name": "a", "seed": 1, "userCount": 1}]}"#,
        RegistryError::InvalidInterestThemeId { index: 0, value: "not-a-uuid".to_owned() }
    )]
    #[case::invalid_safety_toggle_uuid(
        r#"{"version": 1, "interestThemeIds": ["3fa85f64-5717-4562-b3fc-2c963f66afa6"], "safetyToggleIds": ["bad"], "seeds": [{"name": "a", "seed": 1, "userCount": 1}]}"#,
        RegistryError::InvalidSafetyToggleId { index: 0, value: "bad".to_owned() }
    )]
    #[case::empty_seeds(
        r#"{"version": 1, "interestThemeIds": ["3fa85f64-5717-4562-b3fc-2c963f66afa6"], "safetyToggleIds": [], "seeds": []}"#,
        RegistryError::EmptySeeds
    )]
    fn rejects_invalid_registry(#[case] json: &str, #[case] expected: RegistryError) {
        let result = SeedRegistry::from_json(json);
        assert_eq!(result, Err(expected));
    }

    #[rstest]
    #[case("3fa85f64-5717-4562-b3fc-2c963f66afa6")]
    #[case("00000000-0000-0000-0000-000000000000")]
    fn accepts_valid_uuid_formats(#[case] uuid_str: &str) {
        let json = format!(
            r#"{{"version": 1, "interestThemeIds": ["{uuid_str}"], "safetyToggleIds": [], "seeds": [{{"name": "a", "seed": 1, "userCount": 1}}]}}"#
        );
        let result = SeedRegistry::from_json(&json);
        assert!(result.is_ok());
    }

    #[test]
    fn seed_definition_getters_work() {
        let registry = SeedRegistry::from_json(VALID_JSON).expect("valid registry");
        let seed = registry.find_seed("snowy-penguin").expect("seed found");

        assert_eq!(seed.name(), "snowy-penguin");
        assert_eq!(seed.seed(), 1234);
        assert_eq!(seed.user_count(), 5);
    }
}
