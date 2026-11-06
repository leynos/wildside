//! ER diagram snapshot generation from migration-backed schemas.
//!
//! This module orchestrates schema introspection, Mermaid rendering, and
//! atomic snapshot writes for traceable documentation artefacts.

use crate::domain::ports::{SchemaSnapshotRepository, SchemaSnapshotRepositoryError};
use crate::domain::render_mermaid_er_diagram;
use crate::outbound::persistence::PostgresSchemaSnapshotRepository;
use cap_std::{ambient_authority, fs::Dir};
use diesel::Connection;
use diesel::pg::PgConnection;
use diesel_migrations::{FileBasedMigrations, MigrationHarness};
use pg_embedded_setup_unpriv::TestCluster;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;
use uuid::Uuid;

const MERMAID_FILENAME: &str = "schema-baseline.mmd";
const SVG_FILENAME: &str = "schema-baseline.svg";
const MERMAID_HEADER: &str = "%% Generated from backend migrations. Do not edit manually.\n";

mod snapshot_io;

/// Output settings for ER diagram snapshot generation.
#[derive(Debug, Clone)]
pub struct SnapshotRequest {
    /// Directory that receives the generated snapshot files.
    pub output_dir: PathBuf,
    /// Whether to render an SVG from the Mermaid source as well.
    pub should_render_svg: bool,
}
impl Default for SnapshotRequest {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("docs/diagrams/er"),
            should_render_svg: true,
        }
    }
}

/// Paths written by a snapshot generation run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotArtifacts {
    /// Path of the written Mermaid source file.
    pub mermaid_path: PathBuf,
    /// Path of the rendered SVG, or `None` when rendering was skipped.
    pub svg_path: Option<PathBuf>,
}

/// Errors surfaced by ER snapshot generation.
#[derive(Debug, Error)]
pub enum SnapshotGenerationError {
    /// The schema diagram could not be loaded from the repository.
    #[error(transparent)]
    Repository(#[from] SchemaSnapshotRepositoryError),
    /// The embedded PostgreSQL cluster or temporary database could not be set up.
    #[error("embedded postgres setup failed: {message}")]
    EmbeddedPostgres {
        /// Debug-formatted error from the embedded PostgreSQL setup.
        message: String,
    },
    /// Connecting to the database or applying migrations from `path` failed.
    #[error("database migration failed ({path}): {message}")]
    Migration {
        /// Migrations directory that was being applied.
        path: PathBuf,
        /// Error reported by the connection or migration harness.
        message: String,
    },
    /// A filesystem operation on the snapshot output failed.
    #[error("filesystem operation failed ({path}): {message}")]
    Io {
        /// Path involved in the failed operation.
        path: PathBuf,
        /// Error text reported by the filesystem call.
        message: String,
    },
    /// The renderer command could not be started.
    #[error("renderer command '{command}' failed to start: {message}")]
    RendererUnavailable {
        /// Renderer command line that was attempted.
        command: String,
        /// Reason the renderer process could not be spawned.
        message: String,
    },
    /// The renderer ran but exited without success.
    #[error("renderer command '{command}' failed (status {status:?}): {stderr}")]
    RendererFailed {
        /// Renderer command line that was run.
        command: String,
        /// Exit code, or `None` when the process was killed by a signal.
        status: Option<i32>,
        /// Trimmed standard error output from the renderer.
        stderr: String,
    },
}

impl SnapshotGenerationError {
    fn io(path: impl Into<PathBuf>, error: impl std::fmt::Display) -> Self {
        Self::Io {
            path: path.into(),
            message: error.to_string(),
        }
    }

    fn migration(path: impl Into<PathBuf>, error: impl std::fmt::Display) -> Self {
        Self::Migration {
            path: path.into(),
            message: error.to_string(),
        }
    }
}

/// Render Mermaid source files to SVG snapshots.
pub trait MermaidRenderer: Send + Sync {
    /// Render the Mermaid file at `input_path` to an SVG at `output_path`.
    fn render_svg(
        &self,
        input_path: &Path,
        output_path: &Path,
    ) -> Result<(), SnapshotGenerationError>;
}

/// Renderer backed by `mmdc` (Mermaid CLI).
#[derive(Debug, Clone)]
pub struct CommandMermaidRenderer {
    command: Vec<OsString>,
    puppeteer_config: Option<PathBuf>,
}

impl CommandMermaidRenderer {
    /// Build a renderer from a command program and arguments.
    pub fn new(
        program: impl Into<OsString>,
        args: impl IntoIterator<Item = impl Into<OsString>>,
    ) -> Self {
        let mut command = vec![program.into()];
        command.extend(args.into_iter().map(Into::into));

        Self {
            command,
            puppeteer_config: default_puppeteer_config(),
        }
    }

    fn command_label(&self) -> String {
        self.command
            .iter()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl Default for CommandMermaidRenderer {
    fn default() -> Self {
        Self::new("pnpm", ["exec", "mmdc"])
    }
}

impl MermaidRenderer for CommandMermaidRenderer {
    fn render_svg(
        &self,
        input_path: &Path,
        output_path: &Path,
    ) -> Result<(), SnapshotGenerationError> {
        let Some(program) = self.command.first() else {
            return Err(SnapshotGenerationError::RendererUnavailable {
                command: "<empty>".to_owned(),
                message: "renderer command is empty".to_owned(),
            });
        };

        let mut command = Command::new(program);
        for arg in self.command.iter().skip(1) {
            command.arg(arg);
        }

        command
            .arg("--input")
            .arg(input_path)
            .arg("--output")
            .arg(output_path)
            .arg("--outputFormat")
            .arg("svg");

        if let Some(config) = &self.puppeteer_config {
            command.arg("--puppeteerConfigFile").arg(config);
        }

        let output =
            command
                .output()
                .map_err(|error| SnapshotGenerationError::RendererUnavailable {
                    command: self.command_label(),
                    message: error.to_string(),
                })?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(SnapshotGenerationError::RendererFailed {
            command: self.command_label(),
            status: output.status.code(),
            stderr,
        })
    }
}

/// Generate snapshots from a migration-backed embedded PostgreSQL instance.
///
/// # Examples
///
/// ```rust,ignore
/// use backend::er_snapshots::{CommandMermaidRenderer, SnapshotRequest, generate_from_migrations};
/// let artifacts =
///     generate_from_migrations(&CommandMermaidRenderer::default(), &SnapshotRequest::default())?;
/// assert!(artifacts.mermaid_path.ends_with("schema-baseline.mmd"));
/// # Ok::<(), backend::er_snapshots::SnapshotGenerationError>(())
/// ```
pub fn generate_from_migrations(
    renderer: &dyn MermaidRenderer,
    request: &SnapshotRequest,
) -> Result<SnapshotArtifacts, SnapshotGenerationError> {
    let cluster =
        TestCluster::new().map_err(|error| SnapshotGenerationError::EmbeddedPostgres {
            message: format!("{error:?}"),
        })?;
    let temp_db_name = format!("er_snapshot_{}", Uuid::new_v4().simple());
    let database = cluster
        .temporary_database(temp_db_name.as_str())
        .map_err(|error| SnapshotGenerationError::EmbeddedPostgres {
            message: format!("{error:?}"),
        })?;

    apply_migrations(database.url(), migrations_directory().as_path())?;
    let repository = PostgresSchemaSnapshotRepository::new(database.url());
    let diagram = repository.load_schema_diagram()?;

    drop(database);
    drop(cluster);

    generate_from_diagram(diagram, renderer, request)
}

/// Generate snapshots from an already-migrated database URL.
///
/// # Examples
///
/// ```rust,ignore
/// use backend::er_snapshots::{
///     CommandMermaidRenderer, SnapshotRequest, generate_from_database_url
/// };
/// let artifacts = generate_from_database_url(
///     "postgres://postgres:postgres@localhost/example",
///     &CommandMermaidRenderer::default(),
///     &SnapshotRequest::default(),
/// )?;
/// assert!(artifacts.mermaid_path.ends_with("schema-baseline.mmd"));
/// # Ok::<(), backend::er_snapshots::SnapshotGenerationError>(())
/// ```
pub fn generate_from_database_url(
    database_url: &str,
    renderer: &dyn MermaidRenderer,
    request: &SnapshotRequest,
) -> Result<SnapshotArtifacts, SnapshotGenerationError> {
    let repository = PostgresSchemaSnapshotRepository::new(database_url);
    generate_from_repository(&repository, renderer, request)
}

/// Generate snapshots from any port implementation.
///
/// # Examples
///
/// ```rust,ignore
/// use backend::domain::ports::FixtureSchemaSnapshotRepository;
/// use backend::er_snapshots::{
///     CommandMermaidRenderer, SnapshotRequest, generate_from_repository
/// };
/// let request = SnapshotRequest { output_dir: std::env::temp_dir().join("example-er-snapshots"),
///     should_render_svg: false };
/// let artifacts = generate_from_repository(
///     &FixtureSchemaSnapshotRepository,
///     &CommandMermaidRenderer::default(),
///     &request,
/// )?;
/// assert!(artifacts.svg_path.is_none());
/// # Ok::<(), backend::er_snapshots::SnapshotGenerationError>(())
/// ```
pub fn generate_from_repository(
    repository: &dyn SchemaSnapshotRepository,
    renderer: &dyn MermaidRenderer,
    request: &SnapshotRequest,
) -> Result<SnapshotArtifacts, SnapshotGenerationError> {
    let diagram = repository.load_schema_diagram()?;
    generate_from_diagram(diagram, renderer, request)
}

fn generate_from_diagram(
    diagram: crate::domain::SchemaDiagram,
    renderer: &dyn MermaidRenderer,
    request: &SnapshotRequest,
) -> Result<SnapshotArtifacts, SnapshotGenerationError> {
    let mermaid = format!("{MERMAID_HEADER}{}", render_mermaid_er_diagram(&diagram));
    snapshot_io::write_snapshots_atomically(mermaid.as_str(), renderer, request)
}

/// Apply all pending Diesel file-based migrations for the given database.
///
/// This delegates migration execution to Diesel's `FileBasedMigrations` and
/// `run_pending_migrations` via `MigrationHarness`, rather than manually
/// executing SQL files, so runtime behaviour stays aligned with the
/// repository's Diesel migration flow.
///
/// # Examples
///
/// ```rust,no_run
/// use std::path::Path;
///
/// use backend::er_snapshots::{SnapshotGenerationError, apply_migrations};
///
/// let database_url = "postgres://postgres:postgres@localhost:5432/wildside";
/// let migrations_dir = Path::new("migrations");
///
/// // Typical Diesel file-based migrations directory layout:
/// // migrations/
/// // ├── 00000000000000_diesel_initial_setup/
/// // │   └── up.sql
/// // ├── 2024-01-01-120000_create_users/
/// // │   └── up.sql
/// // └── 2024-01-02-150000_add_routes/
/// //     └── up.sql
/// let result = apply_migrations(database_url, migrations_dir);
/// assert!(result.is_ok(), "expected Ok(()), got: {result:?}");
/// # Ok::<(), SnapshotGenerationError>(())
/// ```
///
/// Returns `Ok(())` when every pending migration is applied successfully.
///
/// Returns `Err(SnapshotGenerationError::Migration { .. })` when migration
/// setup or execution fails, including database connection errors, missing or
/// unreadable migrations directories, and SQL execution failures.
pub fn apply_migrations(
    database_url: &str,
    migrations_dir: &Path,
) -> Result<(), SnapshotGenerationError> {
    let mut connection = PgConnection::establish(database_url)
        .map_err(|error| SnapshotGenerationError::migration(migrations_dir, error))?;
    let migrations = FileBasedMigrations::from_path(migrations_dir)
        .map_err(|error| SnapshotGenerationError::migration(migrations_dir, error))?;
    connection
        .run_pending_migrations(migrations)
        .map_err(|error| SnapshotGenerationError::migration(migrations_dir, error))?;
    Ok(())
}

fn migrations_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations")
}

fn default_puppeteer_config() -> Option<PathBuf> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("mmdc-puppeteer.json");
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path.file_name()?;
    let directory = Dir::open_ambient_dir(parent, ambient_authority()).ok()?;
    directory.is_file(file_name).then_some(path)
}

#[cfg(test)]
#[path = "er_snapshots_tests.rs"]
mod tests;
