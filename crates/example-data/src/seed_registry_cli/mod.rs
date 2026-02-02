//! CLI support for updating the seed registry.
//!
//! This module provides parsing and update helpers for the seed registry CLI.
//! The binary delegates to these functions so they can be exercised in tests
//! without spawning a subprocess.

use std::fmt;

use base_d::{WordDictionary, word, wordlists};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs::Dir};
use rand::random;

use crate::error::RegistryError;
use crate::registry::{SeedDefinition, SeedRegistry};

mod error;
pub use error::CliError;

const DEFAULT_USER_COUNT: usize = 12;
const MAX_NAME_ATTEMPTS: usize = 5;

/// Parsed options for the seed registry CLI.
#[derive(Debug, Clone)]
pub struct Options {
    registry_path: Utf8PathBuf,
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
    /// assert!(options.registry_path().as_str().ends_with("seeds.json"));
    /// ```
    #[must_use]
    pub fn registry_path(&self) -> &Utf8Path {
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
    let mut state = ParseState {
        registry_path: None,
        seed: None,
        name: None,
        user_count: None,
    };

    while let Some(arg) = args.next() {
        match handle_flag(&arg, &mut args, &mut state) {
            FlagOutcome::Continue => {}
            FlagOutcome::Help(outcome) => return Ok(outcome),
            FlagOutcome::Error(err) => return Err(err),
        }
    }

    let resolved_registry_path = state.registry_path.ok_or(CliError::MissingRegistryPath)?;
    Ok(ParseOutcome::Options(Options {
        registry_path: resolved_registry_path,
        seed: state.seed,
        name: state.name,
        user_count: state.user_count,
    }))
}

struct ParseState {
    registry_path: Option<Utf8PathBuf>,
    seed: Option<u64>,
    name: Option<String>,
    user_count: Option<usize>,
}

enum FlagOutcome {
    Continue,
    Help(ParseOutcome),
    Error(CliError),
}

fn handle_string_flag<I>(
    args: &mut I,
    flag: &'static str,
    target: &mut Option<impl From<String>>,
) -> FlagOutcome
where
    I: Iterator<Item = String>,
{
    match next_value(args, flag) {
        Ok(value) => {
            *target = Some(value.into());
            FlagOutcome::Continue
        }
        Err(err) => FlagOutcome::Error(err),
    }
}

fn handle_numeric_flag<I, T>(
    args: &mut I,
    flag: &'static str,
    target: &mut Option<T>,
) -> FlagOutcome
where
    I: Iterator<Item = String>,
    T: std::str::FromStr,
    T::Err: fmt::Display,
{
    match next_value(args, flag) {
        Ok(value) => match parse_number(&value, flag) {
            Ok(parsed) => {
                *target = Some(parsed);
                FlagOutcome::Continue
            }
            Err(err) => FlagOutcome::Error(err),
        },
        Err(err) => FlagOutcome::Error(err),
    }
}

fn handle_flag<I>(arg: &str, args: &mut I, state: &mut ParseState) -> FlagOutcome
where
    I: Iterator<Item = String>,
{
    match arg {
        "-h" | "--help" => FlagOutcome::Help(ParseOutcome::Help),
        "--registry" => handle_string_flag(args, "--registry", &mut state.registry_path),
        "--seed" => handle_numeric_flag(args, "--seed", &mut state.seed),
        "--name" => handle_string_flag(args, "--name", &mut state.name),
        "--user-count" => handle_numeric_flag(args, "--user-count", &mut state.user_count),
        _ => FlagOutcome::Error(CliError::UnknownArgument {
            value: arg.to_owned(),
        }),
    }
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
    let registry_dir = open_registry_dir(&options.registry_path)?;
    let file_name = registry_file_name(&options.registry_path)?;
    let registry = SeedRegistry::from_file(&registry_dir, file_name)?;
    let selection = select_seed_and_name(&registry, options, None)?;
    let user_count = options.user_count.unwrap_or(DEFAULT_USER_COUNT);
    let seed_def = SeedDefinition::new(selection.name.clone(), selection.seed, user_count);
    let updated = registry.append_seed(seed_def)?;

    updated.write_to_file(&registry_dir, file_name)?;

    Ok(Update {
        name: selection.name,
        seed: selection.seed,
        user_count,
    })
}

fn open_registry_dir(path: &Utf8Path) -> Result<Dir, CliError> {
    let parent = path.parent().unwrap_or_else(|| Utf8Path::new("."));
    let parent_dir = if parent.as_str().is_empty() {
        Utf8Path::new(".")
    } else {
        parent
    };
    Dir::open_ambient_dir(parent_dir, ambient_authority()).map_err(|err| CliError::RegistryError {
        source: RegistryError::IoError {
            path: path.to_path_buf(),
            message: err.to_string(),
        },
    })
}

fn registry_file_name(path: &Utf8Path) -> Result<&Utf8Path, CliError> {
    let file_name = path.file_name().ok_or_else(|| CliError::RegistryError {
        source: RegistryError::IoError {
            path: path.to_path_buf(),
            message: "registry path must be a file".to_owned(),
        },
    })?;
    Ok(Utf8Path::new(file_name))
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
/// use camino::Utf8Path;
///
/// let update = Update {
///     name: "mossy-owl".to_string(),
///     seed: 2026,
///     user_count: 12,
/// };
/// let message = success_message(&update, Utf8Path::new("seeds.json"));
///
/// assert!(message.contains("mossy-owl"));
/// ```
#[must_use]
pub fn success_message(update: &Update, registry_path: &Utf8Path) -> String {
    format!(
        "Added seed \"{}\" (seed={}, userCount={}) to {}",
        update.name,
        update.seed,
        update.user_count,
        registry_path.as_str()
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
        if !has_name_in_registry(registry, &name) {
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
fn has_name_in_registry(registry: &SeedRegistry, name: &str) -> bool {
    registry.seeds().iter().any(|seed| seed.name() == name)
}
fn random_seed() -> u64 {
    random()
}
fn eff_long_dictionary() -> Result<WordDictionary, CliError> {
    WordDictionary::builder()
        .words_from_str(wordlists::EFF_LONG)
        .delimiter("-")
        .case_sensitive(false)
        .build()
        .map_err(|err| CliError::WordListError { message: err })
}
#[cfg(test)]
mod tests;
