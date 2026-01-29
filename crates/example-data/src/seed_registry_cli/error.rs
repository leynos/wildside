//! Error types for the seed registry CLI.

use thiserror::Error;

use crate::error::RegistryError;

/// Errors surfaced by the CLI parsing and update flow.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CliError {
    /// Registry path was not supplied.
    #[error("missing required flag: --registry")]
    MissingRegistryPath,
    /// A flag expected a value but none was provided.
    #[error("missing value for {flag}")]
    MissingValue {
        /// Flag that was missing its value.
        flag: &'static str,
    },
    /// An unsupported argument was supplied.
    #[error("unknown argument: {value}")]
    UnknownArgument {
        /// Argument value that was not recognized.
        value: String,
    },
    /// A numeric value failed to parse.
    #[error("invalid number for {flag}: '{value}' ({message})")]
    InvalidNumber {
        /// Flag associated with the invalid number.
        flag: &'static str,
        /// Raw value supplied for the flag.
        value: String,
        /// Parser error message.
        message: String,
    },
    /// The EFF word list could not be built.
    #[error("word list error: {message}")]
    WordListError {
        /// Error message describing the failure.
        message: String,
    },
    /// The generated name already exists in the registry.
    #[error("generated seed name '{name}' already exists; supply --name")]
    DuplicateGeneratedName {
        /// Generated name that collided.
        name: String,
    },
    /// Name generation ran out of retries.
    #[error("failed to generate a unique seed name after {attempts} attempts")]
    NameGenerationExhausted {
        /// Number of attempts made.
        attempts: usize,
    },
    /// An error occurred while reading or writing the registry.
    #[error("registry error: {source}")]
    RegistryError {
        /// Underlying registry error.
        #[from]
        #[source]
        source: RegistryError,
    },
}
