//! Scenario-world methods for OSM ingestion BDD tests.

use std::path::Path;
use std::sync::Arc;

use backend::domain::OsmIngestionCommandService;
use backend::domain::ports::{OsmIngestionCommand, OsmIngestionRequest};
use backend::outbound::persistence::{DbPool, DieselOsmIngestionProvenanceRepository, PoolConfig};
use mockable::DefaultClock;
use postgres::NoTls;
use tokio::runtime::Runtime;

use crate::support::atexit_cleanup::shared_cluster_handle;
use crate::support::{drop_table, handle_cluster_setup_failure, provision_template_database};
use crate::{
    DatabaseHandle, FixtureOsmSource, GEOFENCE_BOUNDS, INPUT_DIGEST, OsmIngestionWorld,
    RuntimeHandle, SOURCE_URL,
};

impl OsmIngestionWorld {
    pub fn setup_command(&self) {
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
            Arc::new(DieselOsmIngestionProvenanceRepository::new(pool.clone())),
            Arc::new(DefaultClock),
        );

        self.runtime.set(RuntimeHandle(Arc::new(runtime)));
        self.command.set(command);
        self.database_url.set(database_url);
        self._database.set(DatabaseHandle(Arc::new(temp_db)));
    }

    pub fn is_skipped(&self) -> bool {
        self.setup_error.get().is_some()
    }

    pub fn execute_async<T>(
        &self,
        operation: impl FnOnce(
            &Runtime,
            &OsmIngestionCommandService<FixtureOsmSource, DieselOsmIngestionProvenanceRepository>,
            &str,
        ) -> T,
    ) -> Option<T> {
        if self.is_skipped() {
            return None;
        }

        let runtime = self.runtime.get().expect("runtime");
        let command = self.command.get().expect("command");
        let database_url = self.database_url.get().expect("database url");
        Some(operation(&runtime.0, &command, database_url.as_str()))
    }

    pub fn run_ingest(&self, geofence_id: &str) {
        let geofence_id = geofence_id.to_owned();
        if let Some(result) = self.execute_async(|runtime, command, _database_url| {
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

    pub fn query_counts(&self) -> Option<(i64, i64)> {
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
        let has_provenance_table = client
            .query_one(
                "SELECT to_regclass('osm_ingestion_provenance') IS NOT NULL",
                &[],
            )
            .expect("provenance table existence query")
            .get::<_, bool>(0);
        let provenance_count = if has_provenance_table {
            client
                .query_one("SELECT COUNT(*) FROM osm_ingestion_provenance", &[])
                .expect("provenance count query")
                .get::<_, i64>(0)
        } else {
            0
        };
        Some((poi_count, provenance_count))
    }

    pub fn query_poi_count(&self) -> Option<i64> {
        if self.is_skipped() {
            return None;
        }

        self.query_counts().map(|(poi_count, _)| poi_count)
    }

    pub fn assert_provenance_row(&self) {
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

    pub fn assert_poi_rows(&self) {
        if self.is_skipped() {
            return;
        }

        let database_url = self.database_url.get().expect("database url");
        let mut client =
            postgres::Client::connect(database_url.as_str(), NoTls).expect("connect postgres");
        let rows = client
            .query(
                "SELECT element_type, id, (location)[0], (location)[1], osm_tags->>'name' \
                 FROM pois ORDER BY id",
                &[],
            )
            .expect("poi rows query");

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get::<_, String>(0), "node");
        assert_eq!(rows[0].get::<_, i64>(1), 100);
        assert_eq!(rows[0].get::<_, f64>(2), -3.20);
        assert_eq!(rows[0].get::<_, f64>(3), 55.95);
        assert_eq!(rows[0].get::<_, String>(4), "Fixture POI");
        assert_eq!(rows[1].get::<_, String>(0), "way");
        assert_eq!(rows[1].get::<_, i64>(1), 200);
        assert_eq!(rows[1].get::<_, f64>(2), -3.19);
        assert_eq!(rows[1].get::<_, f64>(3), 55.96);
        assert_eq!(rows[1].get::<_, String>(4), "Fixture POI");
    }

    pub fn drop_provenance_table(&self) {
        if self.is_skipped() {
            return;
        }
        let database_url = self.database_url.get().expect("database url");
        drop_table(database_url.as_str(), "osm_ingestion_provenance")
            .expect("drop table should succeed");
    }
}
