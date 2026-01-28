//! CLI support for updating the seed registry.
//!
//! This module provides parsing and update helpers for the seed registry CLI.
//! The binary delegates to these functions so they can be exercised in tests
//! without spawning a subprocess.

use std::fmt;
use std::path::{Path, PathBuf};

use base_d::{WordDictionary, word, wordlists};
use rand::Rng;
use thiserror::Error;

use crate::error::RegistryError;
use crate::registry::{SeedDefinition, SeedRegistry};

const DEFAULT_USER_COUNT: usize = 12;
const MAX_NAME_ATTEMPTS: usize = 5;

/// Parsed options for the seed registry CLI.
#[derive(Debug, Clone)]
pub struct Options {
    registry_path: PathBuf,
    seed: Option<u64>,
    name: Option<String>,
    user_count: Option<usize>,
}

impl Options {
    /// Returns the registry path supplied for the update.
    ///
    /// # Example
    ///
    /// ```
    /// use example_data::seed_registry_cli::{ParseOutcome, parse_args};
    ///
    /// let args = vec!["--registry".to_string(), "seeds.json".to_string()];
    /// let ParseOutcome::Options(options) = parse_args(args.into_iter()).expect("parse") else {
    ///     panic!("expected options");
    /// };
    ///
    /// assert!(options.registry_path().ends_with("seeds.json"));
    /// ```
    #[must_use]
    pub fn registry_path(&self) -> &Path {
        &self.registry_path
    }
}

/// Outcome of parsing CLI arguments.
#[derive(Debug, Clone)]
pub enum ParseOutcome {
    /// Show help output and exit successfully.
    Help,
    /// Continue with the parsed options.
    Options(Options),
}

/// Result of adding a seed to the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Update {
    /// Name of the seed that was added.
    pub name: String,
    /// RNG seed value stored in the registry.
    pub seed: u64,
    /// User count stored in the registry.
    pub user_count: usize,
}

/// Parses CLI arguments into an update plan.
///
/// # Errors
///
/// Returns [`CliError`] when required flags are missing or values cannot be
/// parsed.
///
/// # Example
///
/// ```
/// use example_data::seed_registry_cli::{ParseOutcome, parse_args};
///
/// let args = vec![
///     "--registry".to_string(),
///     "seeds.json".to_string(),
///     "--name".to_string(),
///     "mossy-owl".to_string(),
/// ];
///
/// let outcome = parse_args(args.into_iter()).expect("parse args");
/// assert!(matches!(outcome, ParseOutcome::Options(_)));
/// ```
pub fn parse_args<I>(mut args: I) -> Result<ParseOutcome, CliError>
where
    I: Iterator<Item = String>,
{
    let mut registry_path: Option<PathBuf> = None;
    let mut seed: Option<u64> = None;
    let mut name: Option<String> = None;
    let mut user_count: Option<usize> = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(ParseOutcome::Help),
            "--registry" => {
                let value = next_value(&mut args, "--registry")?;
                registry_path = Some(PathBuf::from(value));
            }
            "--seed" => {
                let value = next_value(&mut args, "--seed")?;
                seed = Some(parse_number(&value, "--seed")?);
            }
            "--name" => {
                let value = next_value(&mut args, "--name")?;
                name = Some(value);
            }
            "--user-count" => {
                let value = next_value(&mut args, "--user-count")?;
                user_count = Some(parse_number(&value, "--user-count")?);
            }
            _ => return Err(CliError::UnknownArgument { value: arg }),
        }
    }

    let resolved_registry_path = registry_path.ok_or(CliError::MissingRegistryPath)?;
    Ok(ParseOutcome::Options(Options {
        registry_path: resolved_registry_path,
        seed,
        name,
        user_count,
    }))
}

/// Applies the registry update and returns the added seed details.
///
/// # Errors
///
/// Returns [`CliError`] when the registry cannot be read or updated.
///
/// # Example
///
/// ```
/// use example_data::seed_registry_cli::{ParseOutcome, apply_update, parse_args};
/// use std::time::{SystemTime, UNIX_EPOCH};
///
/// let json = r#"{
///     "version": 1,
///     "interestThemeIds": ["3fa85f64-5717-4562-b3fc-2c963f66afa6"],
///     "safetyToggleIds": [],
///     "seeds": [{"name": "mossy-owl", "seed": 2026, "userCount": 12}]
/// }"#;
/// let suffix = SystemTime::now()
///     .duration_since(UNIX_EPOCH)
///     .map(|elapsed| elapsed.as_nanos())
///     .unwrap_or(0);
/// let dir = std::env::temp_dir().join(format!("example-data-cli-{suffix}"));
/// std::fs::create_dir_all(&dir).expect("create temp dir");
/// let path = dir.join("seeds.json");
/// std::fs::write(&path, json).expect("write registry");
///
/// let args = vec![
///     "--registry".to_string(),
///     path.to_string_lossy().to_string(),
///     "--name".to_string(),
///     "river-stone".to_string(),
/// ];
/// let ParseOutcome::Options(options) = parse_args(args.into_iter()).expect("parse") else {
///     panic!("expected options");
/// };
///
/// let update = apply_update(&options).expect("apply update");
///
/// assert_eq!(update.name, "river-stone");
/// std::fs::remove_file(&path).expect("clean up");
/// ```
pub fn apply_update(options: &Options) -> Result<Update, CliError> {
    let registry = SeedRegistry::from_file(&options.registry_path)?;
    let selection = select_seed_and_name(&registry, options, None)?;
    let user_count = options.user_count.unwrap_or(DEFAULT_USER_COUNT);
    let seed_def = SeedDefinition::new(selection.name.clone(), selection.seed, user_count);
    let updated = registry.append_seed(seed_def)?;

    updated.write_to_file(&options.registry_path)?;

    Ok(Update {
        name: selection.name,
        seed: selection.seed,
        user_count,
    })
}

/// Generates a seed name for the supplied seed value.
///
/// # Errors
///
/// Returns [`CliError`] if the word list cannot be loaded.
///
/// # Example
///
/// ```
/// use example_data::seed_registry_cli::seed_name_for_seed;
///
/// let name = seed_name_for_seed(2026).expect("name should generate");
/// assert!(!name.is_empty());
/// ```
pub fn seed_name_for_seed(seed: u64) -> Result<String, CliError> {
    let dictionary = eff_long_dictionary()?;
    Ok(seed_name_from_value(seed, &dictionary))
}

/// Formats the success message emitted by the CLI.
///
/// # Example
///
/// ```
/// use example_data::seed_registry_cli::{Update, success_message};
/// use std::path::Path;
///
/// let update = Update {
///     name: "mossy-owl".to_string(),
///     seed: 2026,
///     user_count: 12,
/// };
/// let message = success_message(&update, Path::new("seeds.json"));
///
/// assert!(message.contains("mossy-owl"));
/// ```
#[must_use]
pub fn success_message(update: &Update, registry_path: &Path) -> String {
    format!(
        "Added seed \"{}\" (seed={}, userCount={}) to {}",
        update.name,
        update.seed,
        update.user_count,
        registry_path.display()
    )
}

fn next_value<I>(args: &mut I, flag: &'static str) -> Result<String, CliError>
where
    I: Iterator<Item = String>,
{
    args.next().ok_or(CliError::MissingValue { flag })
}

fn parse_number<T>(value: &str, flag: &'static str) -> Result<T, CliError>
where
    T: std::str::FromStr,
    T::Err: fmt::Display,
{
    value.parse::<T>().map_err(|err| CliError::InvalidNumber {
        flag,
        value: value.to_owned(),
        message: err.to_string(),
    })
}

#[derive(Debug, Clone)]
struct SeedSelection {
    name: String,
    seed: u64,
}

fn select_seed_and_name(
    registry: &SeedRegistry,
    options: &Options,
    dictionary_opt: Option<WordDictionary>,
) -> Result<SeedSelection, CliError> {
    if let Some(name) = options.name.clone() {
        let seed = options.seed.unwrap_or_else(random_seed);
        return Ok(SeedSelection { name, seed });
    }

    let dictionary = match dictionary_opt {
        Some(dictionary) => dictionary,
        None => eff_long_dictionary()?,
    };

    let supplied_seed = options.seed;
    let mut seed = supplied_seed.unwrap_or_else(random_seed);

    for _ in 0..MAX_NAME_ATTEMPTS {
        let name = seed_name_from_value(seed, &dictionary);
        if !registry_contains_name(registry, &name) {
            return Ok(SeedSelection { name, seed });
        }
        if supplied_seed.is_some() {
            return Err(CliError::DuplicateGeneratedName { name });
        }
        seed = random_seed();
    }

    Err(CliError::NameGenerationExhausted {
        attempts: MAX_NAME_ATTEMPTS,
    })
}

fn seed_name_from_value(seed: u64, dictionary: &WordDictionary) -> String {
    let seed_str = seed.to_string();
    word::encode(seed_str.as_bytes(), dictionary)
}

fn registry_contains_name(registry: &SeedRegistry, name: &str) -> bool {
    registry.seeds().iter().any(|seed| seed.name() == name)
}

fn random_seed() -> u64 {
    rand::rng().random()
}

fn eff_long_dictionary() -> Result<WordDictionary, CliError> {
    WordDictionary::builder()
        .words_from_str(wordlists::EFF_LONG)
        .delimiter("-")
        .case_sensitive(false)
        .build()
        .map_err(|err| CliError::WordListError { message: err })
}

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
        /// Argument value that was not recognised.
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

#[cfg(test)]
mod tests;
