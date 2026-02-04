//! Startup seeding orchestration.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use cap_std::{ambient_authority, fs::Dir};
use example_data::{RegistryError, SeedRegistry};
use mockable::DefaultClock;
use thiserror::Error;
use tracing::{info, warn};

use crate::domain::{ExampleDataSeedOutcome, ExampleDataSeeder, ExampleDataSeedingError};
use crate::example_data::config::ExampleDataSettings;
use crate::outbound::persistence::{DbPool, DieselExampleDataSeedRepository};

/// Errors returned while executing startup seeding.
#[derive(Debug, Error)]
pub enum StartupSeedingError {
    /// Registry file could not be read.
    #[error("failed to read registry at {path}: {source}")]
    RegistryRead {
        /// Path to the registry file.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// Registry parsing failed.
    #[error("registry parse error: {0}")]
    Registry(#[from] RegistryError),
    /// Seed generation or persistence failed.
    #[error("example data seeding error: {0}")]
    Seeding(#[from] ExampleDataSeedingError),
    /// Seed name must not be empty.
    #[error("seed name must not be empty")]
    EmptySeedName,
}

/// Apply example data on startup when enabled.
///
/// # Examples
///
/// ```rust,no_run
/// use std::path::PathBuf;
///
/// use backend::example_data::{ExampleDataSettings, seed_example_data_on_startup};
///
/// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
/// let settings = ExampleDataSettings {
///     is_enabled: false,
///     seed_name: Some("mossy-owl".to_string()),
///     count: None,
///     registry_path: Some(PathBuf::from("fixtures/example-data/seeds.json")),
/// };
/// let outcome = seed_example_data_on_startup(&settings, None).await?;
/// assert!(outcome.is_none());
/// # Ok(())
/// # }
/// ```
pub async fn seed_example_data_on_startup(
    settings: &ExampleDataSettings,
    db_pool: Option<&DbPool>,
) -> Result<Option<ExampleDataSeedOutcome>, StartupSeedingError> {
    if !settings.is_enabled() {
        info!(reason = "disabled", "example data seeding skipped");
        return Ok(None);
    }

    let seed_name = settings.seed_name().trim();
    if seed_name.is_empty() {
        return Err(StartupSeedingError::EmptySeedName);
    }

    let Some(db_pool) = db_pool else {
        warn!(
            seed_key = seed_name,
            "example data seeding enabled but DATABASE_URL is missing; skipping"
        );
        return Ok(None);
    };

    let registry_path = settings.registry_path();
    let registry = load_registry(&registry_path)?;

    let repository = DieselExampleDataSeedRepository::new(db_pool.clone());
    let seeder = ExampleDataSeeder::new(Arc::new(repository), Arc::new(DefaultClock));
    let outcome = seeder
        .seed_from_registry(&registry, seed_name, settings.count)
        .await?;

    match outcome.result {
        crate::domain::ports::SeedingResult::Applied => {
            info!(
                seed_key = %outcome.seed_key,
                user_count = outcome.user_count,
                "example data seeding applied"
            );
        }
        crate::domain::ports::SeedingResult::AlreadySeeded => {
            info!(
                seed_key = %outcome.seed_key,
                user_count = outcome.user_count,
                "example data seed already applied; skipping"
            );
        }
    }

    Ok(Some(outcome))
}

fn load_registry(path: &Path) -> Result<SeedRegistry, StartupSeedingError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let parent = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };
    let file_name = path
        .file_name()
        .ok_or_else(|| StartupSeedingError::RegistryRead {
            path: path.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "registry path must be a file",
            ),
        })?;
    let dir = Dir::open_ambient_dir(parent, ambient_authority()).map_err(|source| {
        StartupSeedingError::RegistryRead {
            path: path.to_path_buf(),
            source,
        }
    })?;
    let payload =
        dir.read(Path::new(file_name))
            .map_err(|source| StartupSeedingError::RegistryRead {
                path: path.to_path_buf(),
                source,
            })?;
    let contents =
        String::from_utf8(payload).map_err(|source| StartupSeedingError::RegistryRead {
            path: path.to_path_buf(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidData, source),
        })?;
    Ok(SeedRegistry::from_json(&contents)?)
}
