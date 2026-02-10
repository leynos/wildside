//! Behavioural tests for ER snapshot generation from migrations.

use std::path::PathBuf;

use backend::er_snapshots::{
    CommandMermaidRenderer, MermaidRenderer, SnapshotArtifacts, SnapshotGenerationError,
    SnapshotRequest, generate_from_database_url,
};
use backend::test_support::cap_fs::{path_exists, read_file_to_string, remove_directory};
use backend::test_support::er_snapshots::FixtureMermaidRenderer;
use cap_std::{ambient_authority, fs::Dir};
use pg_embedded_setup_unpriv::TemporaryDatabase;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use uuid::Uuid;

#[path = "support/pg_embed.rs"]
mod pg_embed;

mod support;

use pg_embed::shared_cluster;
use support::{handle_cluster_setup_failure, provision_template_database};

#[derive(Debug)]
struct SnapshotWorld {
    database: Option<TemporaryDatabase>,
    output_dir: PathBuf,
    result: Option<Result<SnapshotArtifacts, SnapshotGenerationError>>,
    first_mermaid: Option<String>,
    second_mermaid: Option<String>,
    setup_error: Option<String>,
}

impl SnapshotWorld {
    fn ready(database: TemporaryDatabase, output_dir: PathBuf) -> Self {
        Self {
            database: Some(database),
            output_dir,
            result: None,
            first_mermaid: None,
            second_mermaid: None,
            setup_error: None,
        }
    }

    fn skipped(reason: String) -> Self {
        let output_dir = std::env::temp_dir().join(format!(
            "backend-er-snapshots-bdd-skip-{}",
            Uuid::new_v4().simple()
        ));
        Self {
            database: None,
            output_dir,
            result: None,
            first_mermaid: None,
            second_mermaid: None,
            setup_error: Some(reason),
        }
    }

    fn is_skipped(&self) -> bool {
        self.setup_error.is_some()
    }
}

impl Drop for SnapshotWorld {
    fn drop(&mut self) {
        if path_exists(&self.output_dir) {
            let _cleanup_result = remove_directory(&self.output_dir);
        }
    }
}

fn run_snapshot_generation(
    world: &SnapshotWorld,
    renderer: &dyn MermaidRenderer,
    should_render_svg: bool,
) -> Result<SnapshotArtifacts, SnapshotGenerationError> {
    let request = SnapshotRequest {
        output_dir: world.output_dir.clone(),
        should_render_svg,
    };
    let database_url = world
        .database
        .as_ref()
        .expect("database should be available")
        .url();

    generate_from_database_url(database_url, renderer, &request)
}

#[fixture]
fn world() -> SnapshotWorld {
    let cluster = match shared_cluster() {
        Ok(cluster) => cluster,
        Err(reason) => {
            let _: Option<()> = handle_cluster_setup_failure(reason.clone());
            return SnapshotWorld::skipped(reason);
        }
    };
    let database = match provision_template_database(cluster).map_err(|error| error.to_string()) {
        Ok(database) => database,
        Err(reason) => {
            let _: Option<()> = handle_cluster_setup_failure(reason.clone());
            return SnapshotWorld::skipped(reason);
        }
    };

    let output_dir = std::env::temp_dir().join(format!(
        "backend-er-snapshots-bdd-{}",
        Uuid::new_v4().simple()
    ));
    Dir::create_ambient_dir_all(&output_dir, ambient_authority())
        .expect("create snapshot output dir");

    SnapshotWorld::ready(database, output_dir)
}

#[given("a migration-backed temporary database")]
fn a_migration_backed_temporary_database(world: &mut SnapshotWorld) {
    let _ = world;
}

#[given("an empty ER snapshot output directory")]
fn an_empty_er_snapshot_output_directory(world: &mut SnapshotWorld) {
    if world.is_skipped() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
        return;
    }

    if path_exists(&world.output_dir) {
        remove_directory(&world.output_dir).expect("clear output dir");
    }
    Dir::create_ambient_dir_all(&world.output_dir, ambient_authority())
        .expect("recreate output dir");
}

#[when("ER snapshots are generated")]
fn er_snapshots_are_generated(world: &mut SnapshotWorld) {
    if world.is_skipped() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
        return;
    }

    let renderer = FixtureMermaidRenderer::default();
    world.result = Some(run_snapshot_generation(world, &renderer, true));
}

#[when("ER snapshots are generated with a missing renderer command")]
fn er_snapshots_are_generated_with_a_missing_renderer_command(world: &mut SnapshotWorld) {
    if world.is_skipped() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
        return;
    }

    let renderer =
        CommandMermaidRenderer::new("command-that-does-not-exist", std::iter::empty::<&str>());
    world.result = Some(run_snapshot_generation(world, &renderer, true));
}

#[when("ER snapshots are generated twice")]
fn er_snapshots_are_generated_twice(world: &mut SnapshotWorld) {
    if world.is_skipped() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
        return;
    }

    let renderer = FixtureMermaidRenderer::default();
    let first =
        run_snapshot_generation(world, &renderer, false).expect("first generation should succeed");
    let second =
        run_snapshot_generation(world, &renderer, false).expect("second generation should succeed");

    world.first_mermaid =
        Some(read_file_to_string(&first.mermaid_path).expect("read first Mermaid snapshot"));
    world.second_mermaid =
        Some(read_file_to_string(&second.mermaid_path).expect("read second Mermaid snapshot"));
}

#[then("Mermaid and SVG snapshot files are created")]
fn mermaid_and_svg_snapshot_files_are_created(world: &mut SnapshotWorld) {
    if world.is_skipped() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
        return;
    }

    let result = world
        .result
        .take()
        .expect("generation result should be captured")
        .expect("generation should succeed");

    assert!(
        path_exists(&result.mermaid_path),
        "Mermaid snapshot should exist"
    );
    let svg_path = result.svg_path.expect("SVG path should be present");
    assert!(path_exists(&svg_path), "SVG snapshot should exist");
}

#[then("generation fails with a renderer error")]
fn generation_fails_with_a_renderer_error(world: &mut SnapshotWorld) {
    if world.is_skipped() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
        return;
    }

    let error = world
        .result
        .take()
        .expect("generation result should be captured")
        .expect_err("generation should fail");

    assert!(
        matches!(error, SnapshotGenerationError::RendererUnavailable { .. }),
        "expected renderer unavailable error, got: {error}"
    );
}

#[then("no snapshot files are written")]
fn no_snapshot_files_are_written(world: &mut SnapshotWorld) {
    if world.is_skipped() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
        return;
    }

    assert!(!path_exists(
        world.output_dir.join("schema-baseline.mmd").as_path()
    ));
    assert!(!path_exists(
        world.output_dir.join("schema-baseline.svg").as_path()
    ));
}

#[then("the Mermaid snapshot content is identical across runs")]
fn the_mermaid_snapshot_content_is_identical_across_runs(world: &mut SnapshotWorld) {
    if world.is_skipped() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
        return;
    }

    let first = world
        .first_mermaid
        .as_ref()
        .expect("first snapshot should be captured");
    let second = world
        .second_mermaid
        .as_ref()
        .expect("second snapshot should be captured");
    assert_eq!(first, second, "snapshot output should be deterministic");
}

#[scenario(
    path = "tests/features/er_snapshots.feature",
    name = "Snapshots are generated from migrated schema"
)]
fn snapshots_are_generated_from_migrated_schema(world: SnapshotWorld) {
    drop(world);
}

#[scenario(
    path = "tests/features/er_snapshots.feature",
    name = "Snapshot generation reports renderer failures"
)]
fn snapshot_generation_reports_renderer_failures(world: SnapshotWorld) {
    drop(world);
}

#[scenario(
    path = "tests/features/er_snapshots.feature",
    name = "Snapshot generation is deterministic across reruns"
)]
fn snapshot_generation_is_deterministic_across_reruns(world: SnapshotWorld) {
    drop(world);
}
