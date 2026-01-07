//! Error types for the example-data crate.
//!
//! This module defines semantic error enums for registry parsing and user
//! generation, following the project's error handling conventions with
//! `thiserror`.

use std::path::PathBuf;

use thiserror::Error;

/// Errors that can occur when parsing or querying a seed registry.
///
/// These errors cover file I/O, JSON parsing, schema validation, and seed
/// lookup failures.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RegistryError {
    /// The registry file could not be read.
    #[error("failed to read registry file at '{path}': {message}")]
    IoError {
        /// Path to the registry file.
        path: PathBuf,
        /// Description of the I/O error.
        message: String,
    },

    /// The registry JSON is malformed or missing required fields.
    #[error("invalid registry JSON: {message}")]
    ParseError {
        /// Description of the parse error.
        message: String,
    },

    /// The registry version is not supported.
    #[error("unsupported registry version: expected {expected}, found {actual}")]
    UnsupportedVersion {
        /// Expected version number.
        expected: u32,
        /// Actual version found in the registry.
        actual: u32,
    },

    /// An interest theme ID is not a valid UUID.
    #[error("invalid interest theme UUID at index {index}: {value}")]
    InvalidInterestThemeId {
        /// Index of the invalid ID in the array.
        index: usize,
        /// The invalid UUID string.
        value: String,
    },

    /// A safety toggle ID is not a valid UUID.
    #[error("invalid safety toggle UUID at index {index}: {value}")]
    InvalidSafetyToggleId {
        /// Index of the invalid ID in the array.
        index: usize,
        /// The invalid UUID string.
        value: String,
    },

    /// The registry contains no seed definitions.
    #[error("registry contains no seed definitions")]
    EmptySeeds,

    /// The requested seed name was not found in the registry.
    #[error("seed '{name}' not found in registry")]
    SeedNotFound {
        /// The seed name that was not found.
        name: String,
    },
}

/// Errors that can occur during user generation.
///
/// These errors indicate failures in the generation process itself, such as
/// inability to produce valid display names or missing registry data.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GenerationError {
    /// Failed to generate a valid display name after maximum retries.
    #[error("failed to generate valid display name after {max_attempts} attempts")]
    DisplayNameGenerationFailed {
        /// Number of attempts made before giving up.
        max_attempts: usize,
    },

    /// The registry contains no interest theme IDs for selection.
    #[error("registry contains no interest theme IDs for selection")]
    NoInterestThemes,

    /// The registry contains no safety toggle IDs for selection.
    #[error("registry contains no safety toggle IDs for selection")]
    NoSafetyToggles,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_error_io_formats_correctly() {
        let err = RegistryError::IoError {
            path: PathBuf::from("/tmp/seeds.json"),
            message: "file not found".to_owned(),
        };
        assert_eq!(
            err.to_string(),
            "failed to read registry file at '/tmp/seeds.json': file not found"
        );
    }

    #[test]
    fn registry_error_parse_formats_correctly() {
        let err = RegistryError::ParseError {
            message: "unexpected token".to_owned(),
        };
        assert_eq!(err.to_string(), "invalid registry JSON: unexpected token");
    }

    #[test]
    fn registry_error_version_formats_correctly() {
        let err = RegistryError::UnsupportedVersion {
            expected: 1,
            actual: 2,
        };
        assert_eq!(
            err.to_string(),
            "unsupported registry version: expected 1, found 2"
        );
    }

    #[test]
    fn registry_error_invalid_interest_theme_formats_correctly() {
        let err = RegistryError::InvalidInterestThemeId {
            index: 2,
            value: "not-a-uuid".to_owned(),
        };
        assert_eq!(
            err.to_string(),
            "invalid interest theme UUID at index 2: not-a-uuid"
        );
    }

    #[test]
    fn registry_error_invalid_safety_toggle_formats_correctly() {
        let err = RegistryError::InvalidSafetyToggleId {
            index: 0,
            value: "bad".to_owned(),
        };
        assert_eq!(
            err.to_string(),
            "invalid safety toggle UUID at index 0: bad"
        );
    }

    #[test]
    fn registry_error_empty_seeds_formats_correctly() {
        let err = RegistryError::EmptySeeds;
        assert_eq!(err.to_string(), "registry contains no seed definitions");
    }

    #[test]
    fn registry_error_seed_not_found_formats_correctly() {
        let err = RegistryError::SeedNotFound {
            name: "mossy-owl".to_owned(),
        };
        assert_eq!(err.to_string(), "seed 'mossy-owl' not found in registry");
    }

    #[test]
    fn generation_error_display_name_formats_correctly() {
        let err = GenerationError::DisplayNameGenerationFailed { max_attempts: 100 };
        assert_eq!(
            err.to_string(),
            "failed to generate valid display name after 100 attempts"
        );
    }

    #[test]
    fn generation_error_no_themes_formats_correctly() {
        let err = GenerationError::NoInterestThemes;
        assert_eq!(
            err.to_string(),
            "registry contains no interest theme IDs for selection"
        );
    }

    #[test]
    fn generation_error_no_toggles_formats_correctly() {
        let err = GenerationError::NoSafetyToggles;
        assert_eq!(
            err.to_string(),
            "registry contains no safety toggle IDs for selection"
        );
    }
}
