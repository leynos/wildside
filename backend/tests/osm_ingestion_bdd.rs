//! Behaviour-driven tests for OSM ingestion command orchestration.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use backend::domain::ports::{
    OsmIngestionCommand, OsmIngestionOutcome, OsmIngestionRequest, OsmIngestionStatus,
    OsmSourcePoi, OsmSourceReport, OsmSourceRepository, OsmSourceRepositoryError,
};
use backend::domain::{ErrorCode, OsmIngestionCommandService};
use backend::outbound::persistence::{
    DbPool, DieselOsmIngestionProvenanceRepository, DieselOsmPoiRepository, PoolConfig,
};
use mockable::DefaultClock;
use pg_embedded_setup_unpriv::TemporaryDatabase;
use postgres::NoTls;
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};
use tokio::runtime::Runtime;

mod support;

use support::atexit_cleanup::shared_cluster_handle;
use support::{drop_table, handle_cluster_setup_failure, provision_template_database};

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
struct DatabaseHandle(#[expect(dead_code)] Arc<TemporaryDatabase>);

#[derive(Default, ScenarioState)]
struct OsmIngestionWorld {
    runtime: Slot<RuntimeHandle>,
    command: Slot<
        OsmIngestionCommandService<
            FixtureOsmSource,
            DieselOsmPoiRepository,
            DieselOsmIngestionProvenanceRepository,
        >,
    >,
    _pool: Slot<DbPool>,
    database_url: Slot<String>,
    last_result: Slot<Result<OsmIngestionOutcome, backend::domain::Error>>,
    baseline_poi_count: Slot<i64>,
    baseline_provenance_count: Slot<i64>,
    _database: Slot<DatabaseHandle>,
    setup_error: Slot<String>,
}

impl OsmIngestionWorld {
    fn setup_command(&self) {
        let runtime = Runtime::new().expect("create runtime");

        let cluster = match shared_cluster_handle() {
            Ok(cluster) => cluster,
            Err(reason) => {
                let message = reason.to_string();
                let _: Option<()> = handle_cluster_setup_failure(&message);
                self.setup_error.set(message);
                return;
            }
        };

        let temp_db = match provision_template_database(cluster) {
            Ok(db) => db,
            Err(error) => {
                let _: Option<()> = handle_cluster_setup_failure(error.to_string());
                self.setup_error.set(error.to_string());
                return;
            }
        };

        let database_url = temp_db.url().to_string();
        let pool = runtime
            .block_on(DbPool::new(PoolConfig::new(&database_url)))
            .expect("create pool");

        let command = OsmIngestionCommandService::new(
            Arc::new(FixtureOsmSource),
            Arc::new(DieselOsmPoiRepository::new(pool.clone())),
            Arc::new(DieselOsmIngestionProvenanceRepository::new(pool.clone())),
            Arc::new(DefaultClock),
        );

        self.runtime.set(RuntimeHandle(Arc::new(runtime)));
        self.command.set(command);
        self._pool.set(pool);
        self.database_url.set(database_url);
        self._database.set(DatabaseHandle(Arc::new(temp_db)));
    }

    fn is_skipped(&self) -> bool {
        self.setup_error.get().is_some()
    }

    fn execute_async<T>(
        &self,
        operation: impl FnOnce(
            &Runtime,
            &OsmIngestionCommandService<
                FixtureOsmSource,
                DieselOsmPoiRepository,
                DieselOsmIngestionProvenanceRepository,
            >,
            &DbPool,
            &str,
        ) -> T,
    ) -> Option<T> {
        if self.is_skipped() {
            return None;
        }

        let runtime = self.runtime.get().expect("runtime");
        let command = self.command.get().expect("command");
        let pool = self._pool.get().expect("pool");
        let database_url = self.database_url.get().expect("database url");
        Some(operation(
            &runtime.0,
            &command,
            &pool,
            database_url.as_str(),
        ))
    }

    fn run_ingest(&self, geofence_id: &str) {
        let geofence_id = geofence_id.to_owned();
        if let Some(result) = self.execute_async(|runtime, command, _pool, _database_url| {
            runtime.block_on(async {
                let request = OsmIngestionRequest {
                    osm_pbf_path: Path::new("fixtures/launch.osm.pbf").to_path_buf(),
                    source_url: SOURCE_URL.to_owned(),
                    geofence_id,
                    geofence_bounds: GEOFENCE_BOUNDS,
                    input_digest: INPUT_DIGEST.to_owned(),
                };
                command.ingest(request).await
            })
        }) {
            self.last_result.set(result);
        }
    }

    fn query_counts(&self) -> Option<(i64, i64)> {
        if self.is_skipped() {
            return None;
        }

        let database_url = self.database_url.get().expect("database url");
        let mut client =
            postgres::Client::connect(database_url.as_str(), NoTls).expect("connect postgres");
        let poi_count = client
            .query_one("SELECT COUNT(*) FROM pois", &[])
            .expect("poi count query")
            .get::<_, i64>(0);
        let provenance_count = client
            .query_one("SELECT COUNT(*) FROM osm_ingestion_provenance", &[])
            .expect("provenance count query")
            .get::<_, i64>(0);
        Some((poi_count, provenance_count))
    }

    fn assert_provenance_row(&self) {
        if self.is_skipped() {
            return;
        }

        let database_url = self.database_url.get().expect("database url");
        let mut client =
            postgres::Client::connect(database_url.as_str(), NoTls).expect("connect postgres");
        let row = client
            .query_one(
                "SELECT source_url, input_digest, bounds_min_lng, bounds_min_lat, bounds_max_lng, bounds_max_lat \
                 FROM osm_ingestion_provenance LIMIT 1",
                &[],
            )
            .expect("provenance row query");
        let source_url: String = row.get(0);
        let input_digest: String = row.get(1);
        let bounds = [
            row.get::<_, f64>(2),
            row.get::<_, f64>(3),
            row.get::<_, f64>(4),
            row.get::<_, f64>(5),
        ];
        assert_eq!(source_url, SOURCE_URL);
        assert_eq!(input_digest, INPUT_DIGEST);
        assert_eq!(bounds, GEOFENCE_BOUNDS);
    }

    fn drop_provenance_table(&self) {
        if self.is_skipped() {
            return;
        }
        let database_url = self.database_url.get().expect("database url");
        drop_table(database_url.as_str(), "osm_ingestion_provenance")
            .expect("drop table should succeed");
    }
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
}

#[scenario(
    path = "tests/features/osm_ingestion.feature",
    name = "OSM ingestion supports execution reruns and missing-schema failures"
)]
fn osm_ingestion_supports_execution_reruns_and_missing_schema_failures(world: OsmIngestionWorld) {
    drop(world);
}
