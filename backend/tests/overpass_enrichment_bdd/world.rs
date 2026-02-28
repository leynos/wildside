//! Scenario-world methods for Overpass enrichment BDD tests.

use std::collections::BTreeMap;
use std::sync::Arc;

use backend::domain::ports::{
    OverpassEnrichmentRequest, OverpassEnrichmentResponse, OverpassEnrichmentSourceError,
    OverpassPoi,
};
use backend::domain::{
    OverpassEnrichmentWorker, OverpassEnrichmentWorkerConfig, OverpassEnrichmentWorkerPorts,
    OverpassEnrichmentWorkerRuntime,
};
use backend::outbound::persistence::{
    DbPool, DieselEnrichmentProvenanceRepository, DieselOsmPoiRepository, PoolConfig,
};
use chrono::{TimeZone, Utc};
use postgres::NoTls;
use tokio::runtime::Runtime;
use uuid::Uuid;

use crate::support::atexit_cleanup::shared_cluster_handle;
use crate::support::{handle_cluster_setup_failure, provision_template_database};
use crate::{
    DatabaseHandle, ImmediateSleeper, LAUNCH_A_BOUNDS, MutableClock, NoJitter,
    OverpassEnrichmentWorld, RecordingEnrichmentMetrics, RuntimeHandle, ScriptedOverpassSource,
};

impl OverpassEnrichmentWorld {
    /// Build and wire a worker instance for one scenario.
    pub fn setup_worker(
        &self,
        config: OverpassEnrichmentWorkerConfig,
        source_data: Vec<Result<OverpassEnrichmentResponse, OverpassEnrichmentSourceError>>,
    ) {
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

        let source = Arc::new(ScriptedOverpassSource::new(source_data));
        let metrics = Arc::new(RecordingEnrichmentMetrics::default());
        let clock = Arc::new(MutableClock::new(
            Utc.with_ymd_and_hms(2026, 2, 26, 12, 0, 0)
                .single()
                .expect("valid fixed time"),
        ));
        let worker = Arc::new(OverpassEnrichmentWorker::with_runtime(
            OverpassEnrichmentWorkerPorts::new(
                source.clone(),
                Arc::new(DieselOsmPoiRepository::new(pool.clone())),
                Arc::new(DieselEnrichmentProvenanceRepository::new(pool)),
                metrics.clone(),
            ),
            clock.clone(),
            OverpassEnrichmentWorkerRuntime {
                sleeper: Arc::new(ImmediateSleeper),
                jitter: Arc::new(NoJitter),
            },
            config,
        ));

        self.runtime.set(RuntimeHandle(Arc::new(runtime)));
        self.worker.set(worker);
        self.database_url.set(database_url);
        self.source.set(source);
        self.metrics.set(metrics);
        self.clock.set(clock);
        self._database.set(DatabaseHandle(Arc::new(temp_db)));
    }

    /// Start from default config, apply a mutation closure, and set up a worker.
    pub fn setup_with_config_and_data(
        &self,
        configure: impl FnOnce(&mut OverpassEnrichmentWorkerConfig),
        source_data: Vec<Result<OverpassEnrichmentResponse, OverpassEnrichmentSourceError>>,
    ) {
        let mut config = self.default_config();
        configure(&mut config);
        self.setup_worker(config, source_data);
    }

    /// Return the default worker configuration used by BDD scenarios.
    pub fn default_config(&self) -> OverpassEnrichmentWorkerConfig {
        OverpassEnrichmentWorkerConfig {
            max_concurrent_calls: 2,
            max_daily_requests: 10_000,
            max_daily_transfer_bytes: 1_073_741_824,
            max_attempts: 1,
            initial_backoff: std::time::Duration::from_millis(50),
            max_backoff: std::time::Duration::from_millis(250),
            circuit_failure_threshold: 3,
            circuit_open_cooldown: std::time::Duration::from_secs(60),
        }
    }

    /// Create a deterministic synthetic source response payload.
    pub fn make_response(
        &self,
        poi_count: usize,
        transfer_bytes: u64,
    ) -> OverpassEnrichmentResponse {
        OverpassEnrichmentResponse {
            transfer_bytes,
            source_url: "https://overpass.example/api/interpreter".to_owned(),
            pois: (0..poi_count)
                .map(|idx| OverpassPoi {
                    element_type: "node".to_owned(),
                    element_id: idx as i64,
                    longitude: -3.20 + idx as f64 * 0.01,
                    latitude: 55.95,
                    tags: BTreeMap::from([("name".to_owned(), format!("BDD POI {idx}"))]),
                })
                .collect(),
        }
    }

    /// Return whether the scenario should be skipped because setup failed.
    pub fn is_skipped(&self) -> bool {
        self.setup_error.get().is_some()
    }

    /// Print the standard skip marker when setup failed and return `true`.
    pub fn skip_if_needed(&self) -> bool {
        if self.is_skipped() {
            eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
            return true;
        }
        false
    }

    /// Run an operation with runtime, worker, and database URL handles.
    pub fn execute_async<T>(
        &self,
        operation: impl FnOnce(&Runtime, &Arc<OverpassEnrichmentWorker>, &str) -> T,
    ) -> Option<T> {
        if self.is_skipped() {
            return None;
        }

        let runtime = self.runtime.get().expect("runtime should be set");
        let worker = self.worker.get().expect("worker should be set");
        let database_url = self.database_url.get().expect("database URL should be set");
        Some(operation(&runtime.0, &worker, database_url.as_str()))
    }

    /// Execute one enrichment job for the Launch-A bounds fixture.
    pub fn run_job(&self) {
        if let Some(result) = self.execute_async(|runtime, worker, _database_url| {
            runtime.block_on(async {
                let request = OverpassEnrichmentRequest {
                    job_id: Uuid::new_v4(),
                    bounding_box: LAUNCH_A_BOUNDS,
                    tags: vec!["amenity".to_owned()],
                };
                worker.process_job(request).await
            })
        }) {
            self.last_result.set(result);
        }
    }

    /// Query the persisted POI count from the scenario database.
    pub fn query_poi_count(&self) -> Option<i64> {
        if self.is_skipped() {
            return None;
        }

        let database_url = self.database_url.get().expect("database URL should be set");
        let mut client =
            postgres::Client::connect(database_url.as_str(), NoTls).expect("connect postgres");
        let count = client
            .query_one("SELECT COUNT(*) FROM pois", &[])
            .expect("poi count query")
            .get::<_, i64>(0);
        Some(count)
    }

    /// Query the persisted enrichment provenance row count.
    pub fn query_provenance_count(&self) -> Option<i64> {
        if self.is_skipped() {
            return None;
        }

        let database_url = self.database_url.get().expect("database URL should be set");
        let mut client =
            postgres::Client::connect(database_url.as_str(), NoTls).expect("connect postgres");
        let count = client
            .query_one("SELECT COUNT(*) FROM overpass_enrichment_provenance", &[])
            .expect("provenance count query")
            .get::<_, i64>(0);
        Some(count)
    }

    /// Query the latest persisted enrichment provenance row.
    pub fn query_latest_provenance(&self) -> Option<Option<(String, String, [f64; 4])>> {
        if self.is_skipped() {
            return None;
        }

        let database_url = self.database_url.get().expect("database URL should be set");
        let mut client =
            postgres::Client::connect(database_url.as_str(), NoTls).expect("connect postgres");

        let row = client
            .query_opt(
                concat!(
                    "SELECT source_url, ",
                    "to_char(imported_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"'), ",
                    "bounds_min_lng, bounds_min_lat, ",
                    "bounds_max_lng, bounds_max_lat ",
                    "FROM overpass_enrichment_provenance ",
                    "ORDER BY imported_at DESC ",
                    "LIMIT 1"
                ),
                &[],
            )
            .expect("latest provenance query");

        Some(row.map(|row| {
            let source_url = row.get::<_, String>(0);
            let imported_at = row.get::<_, String>(1);
            let bounding_box = [
                row.get::<_, f64>(2),
                row.get::<_, f64>(3),
                row.get::<_, f64>(4),
                row.get::<_, f64>(5),
            ];
            (source_url, imported_at, bounding_box)
        }))
    }

    /// Drop the provenance table to force persistence failures.
    pub fn drop_provenance_table(&self) {
        if self.is_skipped() {
            return;
        }

        let database_url = self.database_url.get().expect("database URL should be set");
        let mut client =
            postgres::Client::connect(database_url.as_str(), NoTls).expect("connect postgres");
        client
            .batch_execute("DROP TABLE overpass_enrichment_provenance")
            .expect("drop provenance table");
    }

    /// Advance the fixture clock by the provided number of seconds.
    pub fn advance_clock_seconds(&self, seconds: i64) {
        if self.is_skipped() {
            return;
        }
        let clock = self.clock.get().expect("clock should be set");
        clock.advance_seconds(seconds);
    }
}
