//! Unit tests for ER snapshot orchestration behaviour.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::{env, io};

use cap_std::{ambient_authority, fs::Dir};
use rstest::rstest;
use uuid::Uuid;

use crate::domain::er_diagram::{SchemaColumn, SchemaDiagram, SchemaRelationship, SchemaTable};
use crate::domain::ports::{SchemaSnapshotRepository, SchemaSnapshotRepositoryError};
use crate::er_snapshots::{
    MermaidRenderer, SnapshotGenerationError, SnapshotRequest, generate_from_repository,
};

#[derive(Debug, Clone)]
struct FixtureRepository {
    diagram: SchemaDiagram,
}

impl SchemaSnapshotRepository for FixtureRepository {
    fn load_schema_diagram(&self) -> Result<SchemaDiagram, SchemaSnapshotRepositoryError> {
        Ok(self.diagram.clone())
    }
}

#[derive(Debug, Clone)]
struct FixtureRenderer {
    fail: bool,
}

impl MermaidRenderer for FixtureRenderer {
    fn render_svg(
        &self,
        _input_path: &std::path::Path,
        output_path: &std::path::Path,
    ) -> Result<(), SnapshotGenerationError> {
        if self.fail {
            return Err(SnapshotGenerationError::RendererFailed {
                command: "fixture-renderer".to_owned(),
                status: Some(1),
                stderr: "fixture failure".to_owned(),
            });
        }

        write_file(output_path, "<svg><text>fixture</text></svg>\n".as_bytes())
            .map_err(|error| SnapshotGenerationError::io(output_path, error))?;
        Ok(())
    }
}

#[rstest]
fn generate_from_repository_writes_mermaid_and_svg() {
    let output_dir = temp_output_dir("writes");
    let repository = FixtureRepository {
        diagram: fixture_diagram(),
    };
    let renderer = FixtureRenderer { fail: false };
    let request = SnapshotRequest {
        output_dir: output_dir.clone(),
        render_svg: true,
    };

    let result = generate_from_repository(&repository, &renderer, &request)
        .expect("snapshot generation should succeed");
    let mermaid = read_file_to_string(&result.mermaid_path).expect("read mermaid snapshot");
    let svg = read_file_to_string(result.svg_path.as_ref().expect("svg path")).expect("read svg");

    assert!(mermaid.starts_with("%% Generated from backend migrations."));
    assert!(mermaid.contains("erDiagram"));
    assert!(svg.contains("<svg>"));
    cleanup(output_dir);
}

#[rstest]
fn generate_from_repository_keeps_output_clean_when_renderer_fails() {
    let output_dir = temp_output_dir("renderer-fails");
    let repository = FixtureRepository {
        diagram: fixture_diagram(),
    };
    let renderer = FixtureRenderer { fail: true };
    let request = SnapshotRequest {
        output_dir: output_dir.clone(),
        render_svg: true,
    };

    let result = generate_from_repository(&repository, &renderer, &request);
    assert!(matches!(
        result,
        Err(SnapshotGenerationError::RendererFailed { .. })
    ));
    assert!(!path_exists(
        output_dir.join("schema-baseline.mmd").as_path()
    ));
    assert!(!path_exists(
        output_dir.join("schema-baseline.svg").as_path()
    ));
    cleanup(output_dir);
}

#[rstest]
fn generate_from_repository_is_deterministic_across_reruns() {
    let output_dir = temp_output_dir("deterministic");
    let repository = FixtureRepository {
        diagram: fixture_diagram(),
    };
    let renderer = FixtureRenderer { fail: false };
    let request = SnapshotRequest {
        output_dir: output_dir.clone(),
        render_svg: false,
    };

    let first = generate_from_repository(&repository, &renderer, &request)
        .expect("first generation should succeed");
    let first_mermaid = read_file_to_string(&first.mermaid_path).expect("read first snapshot");

    let second = generate_from_repository(&repository, &renderer, &request)
        .expect("second generation should succeed");
    let second_mermaid = read_file_to_string(&second.mermaid_path).expect("read second snapshot");

    assert_eq!(first_mermaid, second_mermaid);
    cleanup(output_dir);
}

fn fixture_diagram() -> SchemaDiagram {
    SchemaDiagram {
        tables: vec![
            SchemaTable {
                name: "users".to_owned(),
                columns: vec![SchemaColumn {
                    name: "id".to_owned(),
                    data_type: "uuid".to_owned(),
                    is_primary_key: true,
                    is_nullable: false,
                }],
            },
            SchemaTable {
                name: "routes".to_owned(),
                columns: vec![
                    SchemaColumn {
                        name: "id".to_owned(),
                        data_type: "uuid".to_owned(),
                        is_primary_key: true,
                        is_nullable: false,
                    },
                    SchemaColumn {
                        name: "user_id".to_owned(),
                        data_type: "uuid".to_owned(),
                        is_primary_key: false,
                        is_nullable: false,
                    },
                ],
            },
        ],
        relationships: vec![SchemaRelationship {
            referencing_table: "routes".to_owned(),
            referencing_column: "user_id".to_owned(),
            referenced_table: "users".to_owned(),
            referenced_column: "id".to_owned(),
            referencing_is_nullable: false,
        }],
    }
}

fn temp_output_dir(prefix: &str) -> PathBuf {
    let path = env::temp_dir().join(format!(
        "backend-er-snapshots-{prefix}-{}",
        Uuid::new_v4().simple()
    ));
    Dir::create_ambient_dir_all(&path, ambient_authority()).expect("create temp output directory");
    path
}

fn cleanup(path: PathBuf) {
    let _cleanup_result = remove_directory(path.as_path());
}

fn read_file_to_string(path: &Path) -> io::Result<String> {
    let (parent, file_name) = parent_and_file_name(path)?;
    let directory = Dir::open_ambient_dir(parent, ambient_authority())?;
    directory.read_to_string(Path::new(&file_name))
}

fn write_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    let (parent, file_name) = parent_and_file_name(path)?;
    let directory = Dir::open_ambient_dir(parent, ambient_authority())?;
    directory.write(Path::new(&file_name), contents)
}

fn path_exists(path: &Path) -> bool {
    let Ok((parent, file_name)) = parent_and_file_name(path) else {
        return false;
    };
    let Ok(directory) = Dir::open_ambient_dir(parent, ambient_authority()) else {
        return false;
    };
    directory.exists(Path::new(&file_name))
}

fn remove_directory(path: &Path) -> io::Result<()> {
    let (parent, directory_name) = parent_and_file_name(path)?;
    let directory = Dir::open_ambient_dir(parent, ambient_authority())?;
    match directory.remove_dir_all(Path::new(&directory_name)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn parent_and_file_name(path: &Path) -> io::Result<(&Path, OsString)> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "path must include a file or directory name",
        )
    })?;
    Ok((parent, file_name.to_os_string()))
}
