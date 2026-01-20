//! Seed registry types and JSON parsing.
//!
//! This module defines the seed registry structure that holds named seed
//! definitions and descriptor IDs. The registry is loaded from JSON and
//! provides deterministic seed lookups.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::atomic_io::write_atomic;
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
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

    /// Returns a new registry with the provided seed appended.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::DuplicateSeedName`] if the seed name already
    /// exists in the registry.
    ///
    /// # Example
    ///
    /// ```
    /// use example_data::{SeedDefinition, SeedRegistry};
    ///
    /// let json = r#"{
    ///     "version": 1,
    ///     "interestThemeIds": ["3fa85f64-5717-4562-b3fc-2c963f66afa6"],
    ///     "safetyToggleIds": [],
    ///     "seeds": [{"name": "mossy-owl", "seed": 2026, "userCount": 12}]
    /// }"#;
    ///
    /// let registry = SeedRegistry::from_json(json).expect("valid registry");
    /// let updated =
    ///     registry.append_seed(SeedDefinition::new("river-stone", 99, 5)).expect("append");
    ///
    /// assert!(updated.find_seed("river-stone").is_ok());
    /// ```
    pub fn append_seed(&self, seed: SeedDefinition) -> Result<Self, RegistryError> {
        if self.seeds.iter().any(|existing| existing.name == seed.name) {
            return Err(RegistryError::DuplicateSeedName { name: seed.name });
        }

        let mut seeds = self.seeds.clone();
        seeds.push(seed);

        Ok(Self {
            version: self.version,
            interest_theme_ids: self.interest_theme_ids.clone(),
            safety_toggle_ids: self.safety_toggle_ids.clone(),
            seeds,
        })
    }

    /// Serialises the registry to pretty JSON.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::SerialisationError`] if the registry cannot
    /// be encoded to JSON.
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
    ///     "seeds": [{"name": "mossy-owl", "seed": 2026, "userCount": 12}]
    /// }"#;
    ///
    /// let registry = SeedRegistry::from_json(json).expect("valid registry");
    /// let rendered = registry.to_json_pretty().expect("render");
    ///
    /// assert!(rendered.contains("\"version\": 1"));
    /// ```
    pub fn to_json_pretty(&self) -> Result<String, RegistryError> {
        serde_json::to_string_pretty(self).map_err(|e| RegistryError::SerialisationError {
            message: e.to_string(),
        })
    }

    /// Writes the registry to disk using an atomic rename.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::WriteError`] if the registry cannot be written.
    ///
    /// # Example
    ///
    /// ```
    /// use example_data::SeedRegistry;
    /// use std::time::{SystemTime, UNIX_EPOCH};
    ///
    /// let json = r#"{
    ///     "version": 1,
    ///     "interestThemeIds": ["3fa85f64-5717-4562-b3fc-2c963f66afa6"],
    ///     "safetyToggleIds": [],
    ///     "seeds": [{"name": "mossy-owl", "seed": 2026, "userCount": 12}]
    /// }"#;
    ///
    /// let registry = SeedRegistry::from_json(json).expect("valid registry");
    /// let suffix = SystemTime::now()
    ///     .duration_since(UNIX_EPOCH)
    ///     .map(|elapsed| elapsed.as_nanos())
    ///     .unwrap_or(0);
    /// let dir = std::env::temp_dir().join(format!("example-data-docs-{suffix}"));
    /// std::fs::create_dir_all(&dir).expect("create temp dir");
    /// let path = dir.join("seeds.json");
    ///
    /// registry.write_to_file(&path).expect("write registry");
    /// let rendered = std::fs::read_to_string(&path).expect("read registry");
    ///
    /// assert!(rendered.contains("\"seeds\""));
    /// std::fs::remove_file(&path).expect("clean up");
    /// ```
    pub fn write_to_file(&self, path: &Path) -> Result<(), RegistryError> {
        let json = self.to_json_pretty()?;
        write_atomic(path, &json)
    }
}

/// A named seed definition for deterministic user generation.
///
/// Each seed has a unique name, an RNG seed value, and a user count that
/// determines how many users to generate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeedDefinition {
    name: String,
    seed: u64,
    user_count: usize,
}

impl SeedDefinition {
    /// Creates a new seed definition.
    ///
    /// # Example
    ///
    /// ```
    /// use example_data::SeedDefinition;
    ///
    /// let seed = SeedDefinition::new("mossy-owl".to_owned(), 2026, 12);
    /// assert_eq!(seed.name(), "mossy-owl");
    /// ```
    #[must_use]
    pub fn new(name: impl Into<String>, seed: u64, user_count: usize) -> Self {
        Self {
            name: name.into(),
            seed,
            user_count,
        }
    }

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
