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
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;
use uuid::Uuid;
const MERMAID_FILENAME: &str = "schema-baseline.mmd";
const SVG_FILENAME: &str = "schema-baseline.svg";
const MERMAID_HEADER: &str = "%% Generated from backend migrations. Do not edit manually.\n";

/// Output settings for ER diagram snapshot generation.
#[derive(Debug, Clone)]
pub struct SnapshotRequest {
    pub output_dir: PathBuf,
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
    pub mermaid_path: PathBuf,
    pub svg_path: Option<PathBuf>,
}

/// Errors surfaced by ER snapshot generation.
#[derive(Debug, Error)]
pub enum SnapshotGenerationError {
    #[error(transparent)]
    Repository(#[from] SchemaSnapshotRepositoryError),
    #[error("embedded postgres setup failed: {message}")]
    EmbeddedPostgres { message: String },
    #[error("database migration failed ({path}): {message}")]
    Migration { path: PathBuf, message: String },
    #[error("filesystem operation failed ({path}): {message}")]
    Io { path: PathBuf, message: String },
    #[error("renderer command '{command}' failed to start: {message}")]
    RendererUnavailable { command: String, message: String },
    #[error("renderer command '{command}' failed (status {status:?}): {stderr}")]
    RendererFailed {
        command: String,
        status: Option<i32>,
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
    write_snapshots_atomically(mermaid.as_str(), renderer, request)
}

/// Apply all pending Diesel file-based migrations for the given database.
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

fn write_snapshots_atomically(
    mermaid: &str,
    renderer: &dyn MermaidRenderer,
    request: &SnapshotRequest,
) -> Result<SnapshotArtifacts, SnapshotGenerationError> {
    Dir::create_ambient_dir_all(&request.output_dir, ambient_authority())
        .map_err(|error| SnapshotGenerationError::io(&request.output_dir, error))?;
    let output_dir = Dir::open_ambient_dir(&request.output_dir, ambient_authority())
        .map_err(|error| SnapshotGenerationError::io(&request.output_dir, error))?;

    let staging_dir_name = format!(".tmp-er-snapshot-{}", Uuid::new_v4().simple());
    output_dir.create_dir(&staging_dir_name).map_err(|error| {
        SnapshotGenerationError::io(request.output_dir.join(&staging_dir_name), error)
    })?;

    let staged_mermaid_relative = PathBuf::from(&staging_dir_name).join(MERMAID_FILENAME);
    let staged_svg_relative = PathBuf::from(&staging_dir_name).join(SVG_FILENAME);
    let staged_mermaid = request.output_dir.join(&staged_mermaid_relative);
    let staged_svg = request.output_dir.join(&staged_svg_relative);
    let final_mermaid = request.output_dir.join(MERMAID_FILENAME);
    let final_svg = request.output_dir.join(SVG_FILENAME);

    let result = (|| -> Result<SnapshotArtifacts, SnapshotGenerationError> {
        output_dir
            .write(&staged_mermaid_relative, mermaid.as_bytes())
            .map_err(|error| SnapshotGenerationError::io(&staged_mermaid, error))?;

        let svg_path = if request.should_render_svg {
            renderer.render_svg(&staged_mermaid, &staged_svg)?;
            Some(final_svg.clone())
        } else {
            None
        };

        replace_file(
            &output_dir,
            staged_mermaid_relative.as_path(),
            Path::new(MERMAID_FILENAME),
            &request.output_dir,
        )?;
        if request.should_render_svg {
            replace_file(
                &output_dir,
                staged_svg_relative.as_path(),
                Path::new(SVG_FILENAME),
                &request.output_dir,
            )?;
        } else {
            remove_file_if_exists(&output_dir, Path::new(SVG_FILENAME), &request.output_dir)?;
        }

        Ok(SnapshotArtifacts {
            mermaid_path: final_mermaid,
            svg_path,
        })
    })();

    let _cleanup_result = output_dir.remove_dir_all(&staging_dir_name);
    result
}

fn replace_file(
    directory: &Dir,
    from: &Path,
    to: &Path,
    output_dir: &Path,
) -> Result<(), SnapshotGenerationError> {
    match directory.remove_file(to) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(SnapshotGenerationError::io(output_dir.join(to), error));
        }
    }
    directory
        .rename(from, directory, to)
        .map_err(|error| SnapshotGenerationError::io(output_dir.join(to), error))
}

fn remove_file_if_exists(
    directory: &Dir,
    path: &Path,
    output_dir: &Path,
) -> Result<(), SnapshotGenerationError> {
    match directory.remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(SnapshotGenerationError::io(output_dir.join(path), error)),
    }
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
