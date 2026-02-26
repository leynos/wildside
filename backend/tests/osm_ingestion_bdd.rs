//! Behaviour-driven tests for OSM ingestion command orchestration.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use backend::domain::ports::{
    OsmIngestionOutcome, OsmIngestionStatus, OsmSourcePoi, OsmSourceReport, OsmSourceRepository,
    OsmSourceRepositoryError,
};
use backend::domain::{ErrorCode, OsmIngestionCommandService};
use backend::outbound::persistence::DieselOsmIngestionProvenanceRepository;
use pg_embedded_setup_unpriv::TemporaryDatabase;
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};
use tokio::runtime::Runtime;

#[path = "osm_ingestion_bdd/world.rs"]
mod osm_ingestion_world;
mod support;

const INPUT_DIGEST: &str = "2e7d2c03a9507ae265ecf5b5356885a53393a2029f7c98f0f8f9f8f2a5f1f7c6";
const SOURCE_URL: &str = "https://example.test/launch.osm.pbf";
const GEOFENCE_BOUNDS: [f64; 4] = [-3.30, 55.90, -3.10, 56.00];

#[derive(Debug, Clone, Copy, Default)]
struct FixtureOsmSource;

#[async_trait]
impl OsmSourceRepository for FixtureOsmSource {
    async fn ingest_osm_pbf(
        &self,
        _path: &Path,
    ) -> Result<OsmSourceReport, OsmSourceRepositoryError> {
        let tags = BTreeMap::from([("name".to_owned(), "Fixture POI".to_owned())]);
        Ok(OsmSourceReport {
            pois: vec![
                OsmSourcePoi {
                    encoded_element_id: 100,
                    longitude: -3.20,
                    latitude: 55.95,
                    tags: tags.clone(),
                },
                OsmSourcePoi {
                    encoded_element_id: (1 << 62) | 200,
                    longitude: -3.19,
                    latitude: 55.96,
                    tags: tags.clone(),
                },
                OsmSourcePoi {
                    encoded_element_id: 300,
                    longitude: -3.50,
                    latitude: 55.80,
                    tags,
                },
            ],
        })
    }
}

#[derive(Clone)]
struct RuntimeHandle(Arc<Runtime>);

#[derive(Clone)]
struct DatabaseHandle(
    #[expect(
        dead_code,
        reason = "hold temp database handle so Drop cleans up cluster resources"
    )]
    Arc<TemporaryDatabase>,
);

#[derive(Default, ScenarioState)]
struct OsmIngestionWorld {
    runtime: Slot<RuntimeHandle>,
    command:
        Slot<OsmIngestionCommandService<FixtureOsmSource, DieselOsmIngestionProvenanceRepository>>,
    database_url: Slot<String>,
    last_result: Slot<Result<OsmIngestionOutcome, backend::domain::Error>>,
    baseline_poi_count: Slot<i64>,
    baseline_provenance_count: Slot<i64>,
    _database: Slot<DatabaseHandle>,
    setup_error: Slot<String>,
}

#[fixture]
fn world() -> OsmIngestionWorld {
    OsmIngestionWorld::default()
}

#[given("a Diesel-backed OSM ingestion command service")]
fn a_diesel_backed_osm_ingestion_command_service(world: &OsmIngestionWorld) {
    world.setup_command();
}

#[when("an ingest run executes for geofence launch-a")]
fn an_ingest_run_executes_for_geofence_launch_a(world: &OsmIngestionWorld) {
    world.run_ingest("launch-a");
}

#[then("the command reports an executed ingest outcome")]
fn the_command_reports_an_executed_ingest_outcome(world: &OsmIngestionWorld) {
    if world.is_skipped() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
        return;
    }

    let result = world.last_result.get().expect("result should be set");
    let outcome = result.as_ref().expect("ingest should succeed");
    assert_eq!(outcome.status, OsmIngestionStatus::Executed);
    assert_eq!(outcome.raw_poi_count, 3);
    assert_eq!(outcome.persisted_poi_count, 2);
    assert_eq!(outcome.geofence_id, "launch-a");
    assert_eq!(outcome.source_url, SOURCE_URL);
    assert_eq!(outcome.input_digest, INPUT_DIGEST);
}

#[then("geofenced POIs and provenance are persisted")]
fn geofenced_pois_and_provenance_are_persisted(world: &OsmIngestionWorld) {
    let Some((poi_count, provenance_count)) = world.query_counts() else {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
        return;
    };
    assert_eq!(poi_count, 2);
    assert_eq!(provenance_count, 1);
    world.assert_poi_rows();
    world.assert_provenance_row();
    world.baseline_poi_count.set(poi_count);
    world.baseline_provenance_count.set(provenance_count);
}

#[when("the same ingest reruns for geofence launch-a and digest")]
fn the_same_ingest_reruns_for_geofence_launch_a_and_digest(world: &OsmIngestionWorld) {
    world.run_ingest("launch-a");
}

#[then("the command reports a replayed ingest outcome")]
fn the_command_reports_a_replayed_ingest_outcome(world: &OsmIngestionWorld) {
    if world.is_skipped() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
        return;
    }

    let result = world.last_result.get().expect("result should be set");
    let outcome = result.as_ref().expect("replay should succeed");
    assert_eq!(outcome.status, OsmIngestionStatus::Replayed);
    assert_eq!(outcome.raw_poi_count, 3);
    assert_eq!(outcome.persisted_poi_count, 2);
}

#[then("persisted row counts stay unchanged")]
fn persisted_row_counts_stay_unchanged(world: &OsmIngestionWorld) {
    let Some((poi_count, provenance_count)) = world.query_counts() else {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
        return;
    };
    assert_eq!(Some(poi_count), world.baseline_poi_count.get());
    assert_eq!(
        Some(provenance_count),
        world.baseline_provenance_count.get()
    );
}

#[when("the provenance table is dropped")]
fn the_provenance_table_is_dropped(world: &OsmIngestionWorld) {
    world.drop_provenance_table();
}

#[when("an ingest run executes for geofence launch-b")]
fn an_ingest_run_executes_for_geofence_launch_b(world: &OsmIngestionWorld) {
    world.run_ingest("launch-b");
}

#[then("the command fails with service unavailable")]
fn the_command_fails_with_service_unavailable(world: &OsmIngestionWorld) {
    if world.is_skipped() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
        return;
    }

    let result = world.last_result.get().expect("result should be set");
    let error = result.as_ref().expect_err("run should fail");
    assert_eq!(error.code(), ErrorCode::ServiceUnavailable);
    let poi_count = world
        .query_poi_count()
        .expect("poi count should be available");
    assert_eq!(
        poi_count, 0,
        "atomic ingest persistence should roll back POI writes on failure"
    );
}

#[scenario(
    path = "tests/features/osm_ingestion.feature",
    name = "OSM ingestion persists geofenced POIs and provenance"
)]
fn osm_ingestion_persists_geofenced_pois_and_provenance(world: OsmIngestionWorld) {
    drop(world);
}

#[scenario(
    path = "tests/features/osm_ingestion.feature",
    name = "OSM ingestion reruns deterministically for the same geofence and digest"
)]
fn osm_ingestion_reruns_deterministically_for_the_same_geofence_and_digest(
    world: OsmIngestionWorld,
) {
    drop(world);
}

#[scenario(
    path = "tests/features/osm_ingestion.feature",
    name = "OSM ingestion fails when provenance persistence is unavailable"
)]
fn osm_ingestion_fails_when_provenance_persistence_is_unavailable(world: OsmIngestionWorld) {
    drop(world);
}
