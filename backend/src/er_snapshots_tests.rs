//! Unit tests for ER snapshot orchestration behaviour.

use std::env;
use std::path::PathBuf;

use cap_std::{ambient_authority, fs::Dir};
use rstest::{fixture, rstest};
use uuid::Uuid;

use crate::domain::er_diagram::{SchemaColumn, SchemaDiagram, SchemaRelationship, SchemaTable};
use crate::domain::ports::{SchemaSnapshotRepository, SchemaSnapshotRepositoryError};
use crate::er_snapshots::{SnapshotGenerationError, SnapshotRequest, generate_from_repository};
use crate::test_support::cap_fs::{path_exists, read_file_to_string, remove_directory, write_file};
use crate::test_support::er_snapshots::FixtureMermaidRenderer;

#[derive(Debug, Clone)]
struct FixtureRepository {
    diagram: SchemaDiagram,
}

impl SchemaSnapshotRepository for FixtureRepository {
    fn load_schema_diagram(&self) -> Result<SchemaDiagram, SchemaSnapshotRepositoryError> {
        Ok(self.diagram.clone())
    }
}

#[fixture]
fn repository() -> FixtureRepository {
    FixtureRepository {
        diagram: fixture_diagram(),
    }
}

#[fixture]
fn successful_renderer() -> FixtureMermaidRenderer {
    FixtureMermaidRenderer { should_fail: false }
}

#[fixture]
fn failing_renderer() -> FixtureMermaidRenderer {
    FixtureMermaidRenderer { should_fail: true }
}

#[rstest]
fn generate_from_repository_writes_mermaid_and_svg(
    repository: FixtureRepository,
    successful_renderer: FixtureMermaidRenderer,
) {
    let output_dir = temp_output_dir("writes");
    let request = SnapshotRequest {
        output_dir: output_dir.clone(),
        should_render_svg: true,
    };

    let result = generate_from_repository(&repository, &successful_renderer, &request)
        .expect("snapshot generation should succeed");
    let mermaid = read_file_to_string(&result.mermaid_path).expect("read mermaid snapshot");
    let svg = read_file_to_string(result.svg_path.as_ref().expect("svg path")).expect("read svg");

    assert!(mermaid.starts_with("%% Generated from backend migrations."));
    assert!(mermaid.contains("erDiagram"));
    assert!(svg.contains("<svg>"));
    cleanup(output_dir);
}

#[rstest]
fn generate_from_repository_keeps_output_clean_when_renderer_fails(
    repository: FixtureRepository,
    failing_renderer: FixtureMermaidRenderer,
) {
    let output_dir = temp_output_dir("renderer-fails");
    let request = SnapshotRequest {
        output_dir: output_dir.clone(),
        should_render_svg: true,
    };

    let result = generate_from_repository(&repository, &failing_renderer, &request);
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
fn generate_from_repository_is_deterministic_across_reruns(
    repository: FixtureRepository,
    successful_renderer: FixtureMermaidRenderer,
) {
    let output_dir = temp_output_dir("deterministic");
    let request = SnapshotRequest {
        output_dir: output_dir.clone(),
        should_render_svg: false,
    };

    let first = generate_from_repository(&repository, &successful_renderer, &request)
        .expect("first generation should succeed");
    let first_mermaid = read_file_to_string(&first.mermaid_path).expect("read first snapshot");

    let second = generate_from_repository(&repository, &successful_renderer, &request)
        .expect("second generation should succeed");
    let second_mermaid = read_file_to_string(&second.mermaid_path).expect("read second snapshot");

    assert_eq!(first_mermaid, second_mermaid);
    cleanup(output_dir);
}

#[rstest]
fn generate_from_repository_removes_stale_svg_when_render_is_disabled(
    repository: FixtureRepository,
    successful_renderer: FixtureMermaidRenderer,
) {
    let output_dir = temp_output_dir("stale-svg");
    let stale_svg_path = output_dir.join("schema-baseline.svg");
    write_file(
        &stale_svg_path,
        "<svg><text>stale</text></svg>\n".as_bytes(),
    )
    .expect("write stale svg fixture");

    let request = SnapshotRequest {
        output_dir: output_dir.clone(),
        should_render_svg: false,
    };

    let result = generate_from_repository(&repository, &successful_renderer, &request)
        .expect("snapshot generation should succeed");

    assert!(
        path_exists(&result.mermaid_path),
        "Mermaid snapshot should exist after generation"
    );
    assert_eq!(None, result.svg_path, "svg path should be omitted");
    assert!(
        !path_exists(&stale_svg_path),
        "stale svg should be removed when svg rendering is disabled"
    );

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
