//! Behaviour-driven tests for Overpass enrichment worker orchestration.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use backend::domain::ports::{
    EnrichmentJobFailure, EnrichmentJobFailureKind, EnrichmentJobMetrics,
    EnrichmentJobMetricsError, EnrichmentJobSuccess, OverpassEnrichmentRequest,
    OverpassEnrichmentResponse, OverpassEnrichmentSource, OverpassEnrichmentSourceError,
};
use backend::domain::{
    BackoffJitter, EnrichmentSleeper, ErrorCode, OverpassEnrichmentJobOutcome,
    OverpassEnrichmentWorker,
};
use chrono::{DateTime, Local, TimeDelta, Utc};
use mockable::Clock;
use pg_embedded_setup_unpriv::TemporaryDatabase;
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};
use std::time::Duration;

#[path = "overpass_enrichment_bdd/world.rs"]
mod overpass_enrichment_world;
mod support;

const LAUNCH_A_BOUNDS: [f64; 4] = [-3.30, 55.90, -3.10, 56.00];

#[derive(Clone)]
struct RuntimeHandle(Arc<tokio::runtime::Runtime>);

#[derive(Clone)]
struct DatabaseHandle(
    #[expect(
        dead_code,
        reason = "hold temp database handle so Drop cleans up cluster resources"
    )]
    Arc<TemporaryDatabase>,
);

struct MutableClock(Mutex<DateTime<Utc>>);

impl MutableClock {
    fn new(now: DateTime<Utc>) -> Self {
        Self(Mutex::new(now))
    }

    fn advance_seconds(&self, seconds: i64) {
        let mut guard = self.0.lock().expect("clock mutex");
        *guard += TimeDelta::seconds(seconds);
    }
}

impl Clock for MutableClock {
    fn local(&self) -> DateTime<Local> {
        self.utc().with_timezone(&Local)
    }

    fn utc(&self) -> DateTime<Utc> {
        *self.0.lock().expect("clock mutex")
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct ImmediateSleeper;

#[async_trait]
impl EnrichmentSleeper for ImmediateSleeper {
    async fn sleep(&self, _duration: Duration) {}
}

#[derive(Debug, Clone, Copy, Default)]
struct NoJitter;

impl BackoffJitter for NoJitter {
    fn jittered_delay(&self, base: Duration, _attempt: u32, _now: DateTime<Utc>) -> Duration {
        base
    }
}

struct ScriptedOverpassSource {
    scripted: Mutex<VecDeque<Result<OverpassEnrichmentResponse, OverpassEnrichmentSourceError>>>,
    calls: AtomicUsize,
}

impl ScriptedOverpassSource {
    fn new(
        scripted: Vec<Result<OverpassEnrichmentResponse, OverpassEnrichmentSourceError>>,
    ) -> Self {
        Self {
            scripted: Mutex::new(scripted.into()),
            calls: AtomicUsize::new(0),
        }
    }

    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl OverpassEnrichmentSource for ScriptedOverpassSource {
    async fn fetch_pois(
        &self,
        _request: &OverpassEnrichmentRequest,
    ) -> Result<OverpassEnrichmentResponse, OverpassEnrichmentSourceError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.scripted
            .lock()
            .expect("source script mutex")
            .pop_front()
            .unwrap_or_else(|| Ok(OverpassEnrichmentResponse::default()))
    }
}

#[derive(Default)]
struct RecordingEnrichmentMetrics {
    successes: Mutex<Vec<EnrichmentJobSuccess>>,
    failures: Mutex<Vec<EnrichmentJobFailure>>,
}

impl RecordingEnrichmentMetrics {
    fn success_count(&self) -> usize {
        self.successes.lock().expect("metrics mutex").len()
    }

    fn failure_kinds(&self) -> Vec<EnrichmentJobFailureKind> {
        self.failures
            .lock()
            .expect("metrics mutex")
            .iter()
            .map(|payload| payload.kind)
            .collect()
    }
}

#[async_trait]
impl EnrichmentJobMetrics for RecordingEnrichmentMetrics {
    async fn record_success(
        &self,
        payload: &EnrichmentJobSuccess,
    ) -> Result<(), EnrichmentJobMetricsError> {
        self.successes
            .lock()
            .expect("metrics mutex")
            .push(payload.clone());
        Ok(())
    }

    async fn record_failure(
        &self,
        payload: &EnrichmentJobFailure,
    ) -> Result<(), EnrichmentJobMetricsError> {
        self.failures
            .lock()
            .expect("metrics mutex")
            .push(payload.clone());
        Ok(())
    }
}

#[derive(Default, ScenarioState)]
struct OverpassEnrichmentWorld {
    runtime: Slot<RuntimeHandle>,
    worker: Slot<Arc<OverpassEnrichmentWorker>>,
    database_url: Slot<String>,
    source: Slot<Arc<ScriptedOverpassSource>>,
    metrics: Slot<Arc<RecordingEnrichmentMetrics>>,
    clock: Slot<Arc<MutableClock>>,
    last_result: Slot<Result<OverpassEnrichmentJobOutcome, backend::domain::Error>>,
    _database: Slot<DatabaseHandle>,
    setup_error: Slot<String>,
}

#[fixture]
fn world() -> OverpassEnrichmentWorld {
    OverpassEnrichmentWorld::default()
}

#[given("a Diesel-backed Overpass enrichment worker with successful source data")]
fn a_diesel_backed_overpass_enrichment_worker_with_successful_source_data(
    world: &OverpassEnrichmentWorld,
) {
    let source_data = vec![Ok(world.make_response(2, 768))];
    world.setup_worker(world.default_config(), source_data);
}

#[given("a Diesel-backed Overpass enrichment worker with exhausted request quota")]
fn a_diesel_backed_overpass_enrichment_worker_with_exhausted_request_quota(
    world: &OverpassEnrichmentWorld,
) {
    let mut config = world.default_config();
    config.max_daily_requests = 0;
    let source_data = vec![Ok(world.make_response(1, 128))];
    world.setup_worker(config, source_data);
}

#[given("a Diesel-backed Overpass enrichment worker with failing source responses")]
fn a_diesel_backed_overpass_enrichment_worker_with_failing_source_responses(
    world: &OverpassEnrichmentWorld,
) {
    let mut config = world.default_config();
    config.max_attempts = 1;
    config.circuit_failure_threshold = 2;
    config.circuit_open_cooldown = Duration::from_secs(300);
    let source_data = vec![
        Err(OverpassEnrichmentSourceError::transport("boom-1")),
        Err(OverpassEnrichmentSourceError::transport("boom-2")),
    ];
    world.setup_worker(config, source_data);
}

#[given("a Diesel-backed Overpass enrichment worker with recovery source responses")]
fn a_diesel_backed_overpass_enrichment_worker_with_recovery_source_responses(
    world: &OverpassEnrichmentWorld,
) {
    let mut config = world.default_config();
    config.max_attempts = 1;
    config.circuit_failure_threshold = 1;
    config.circuit_open_cooldown = Duration::from_secs(60);
    let source_data = vec![
        Err(OverpassEnrichmentSourceError::transport("boom-once")),
        Ok(world.make_response(1, 256)),
    ];
    world.setup_worker(config, source_data);
}

#[when("an enrichment job runs for launch-a bounds")]
fn an_enrichment_job_runs_for_launch_a_bounds(world: &OverpassEnrichmentWorld) {
    world.run_job();
}

#[when("two enrichment jobs run for launch-a bounds")]
fn two_enrichment_jobs_run_for_launch_a_bounds(world: &OverpassEnrichmentWorld) {
    world.run_job();
    world.run_job();
}

#[when("a third enrichment job runs for launch-a bounds")]
fn a_third_enrichment_job_runs_for_launch_a_bounds(world: &OverpassEnrichmentWorld) {
    world.run_job();
}

#[when("one enrichment job fails for launch-a bounds")]
fn one_enrichment_job_fails_for_launch_a_bounds(world: &OverpassEnrichmentWorld) {
    world.run_job();
    if world.is_skipped() {
        return;
    }
    let result = world.last_result.get().expect("last result should exist");
    result.as_ref().expect_err("first job should fail");
}

#[when("the worker clock advances by 61 seconds")]
fn the_worker_clock_advances_by_61_seconds(world: &OverpassEnrichmentWorld) {
    world.advance_clock_seconds(61);
}

#[then("the worker reports a successful enrichment outcome")]
fn the_worker_reports_a_successful_enrichment_outcome(world: &OverpassEnrichmentWorld) {
    if world.is_skipped() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
        return;
    }

    let result = world.last_result.get().expect("last result should be set");
    let outcome = result.as_ref().expect("job should succeed");
    assert!(outcome.persisted_poi_count >= 1);
    assert!(outcome.transfer_bytes >= 1);
}

#[then("enrichment POIs are persisted")]
fn enrichment_pois_are_persisted(world: &OverpassEnrichmentWorld) {
    let Some(poi_count) = world.query_poi_count() else {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
        return;
    };
    assert!(poi_count >= 1);
}

#[then("an enrichment success metric is recorded")]
fn an_enrichment_success_metric_is_recorded(world: &OverpassEnrichmentWorld) {
    if world.is_skipped() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
        return;
    }

    let metrics = world.metrics.get().expect("metrics should be set");
    assert_eq!(metrics.success_count(), 1);
}

#[then("the worker fails with service unavailable")]
fn the_worker_fails_with_service_unavailable(world: &OverpassEnrichmentWorld) {
    if world.is_skipped() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
        return;
    }

    let result = world.last_result.get().expect("last result should be set");
    let error = result.as_ref().expect_err("job should fail");
    assert_eq!(error.code(), ErrorCode::ServiceUnavailable);
}

#[then("no Overpass source calls were made")]
fn no_overpass_source_calls_were_made(world: &OverpassEnrichmentWorld) {
    if world.is_skipped() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
        return;
    }
    let source = world.source.get().expect("source should be set");
    assert_eq!(source.call_count(), 0);
}

#[then("an enrichment quota failure metric is recorded")]
fn an_enrichment_quota_failure_metric_is_recorded(world: &OverpassEnrichmentWorld) {
    if world.is_skipped() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
        return;
    }

    let metrics = world.metrics.get().expect("metrics should be set");
    assert!(
        metrics
            .failure_kinds()
            .contains(&EnrichmentJobFailureKind::QuotaRequestLimit),
        "expected quota failure metric kind"
    );
}

#[then("the third job fails fast with service unavailable")]
fn the_third_job_fails_fast_with_service_unavailable(world: &OverpassEnrichmentWorld) {
    the_worker_fails_with_service_unavailable(world);
}

#[then("the source call count is two")]
fn the_source_call_count_is_two(world: &OverpassEnrichmentWorld) {
    if world.is_skipped() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
        return;
    }
    let source = world.source.get().expect("source should be set");
    assert_eq!(source.call_count(), 2);
}

#[then("an enrichment circuit-open metric is recorded")]
fn an_enrichment_circuit_open_metric_is_recorded(world: &OverpassEnrichmentWorld) {
    if world.is_skipped() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped");
        return;
    }

    let metrics = world.metrics.get().expect("metrics should be set");
    assert!(
        metrics
            .failure_kinds()
            .contains(&EnrichmentJobFailureKind::CircuitOpen),
        "expected circuit-open failure metric"
    );
}

#[scenario(
    path = "tests/features/overpass_enrichment.feature",
    name = "Overpass enrichment persists fetched POIs"
)]
fn overpass_enrichment_persists_fetched_pois(world: OverpassEnrichmentWorld) {
    drop(world);
}

#[scenario(
    path = "tests/features/overpass_enrichment.feature",
    name = "Overpass enrichment respects request quota limits"
)]
fn overpass_enrichment_respects_request_quota_limits(world: OverpassEnrichmentWorld) {
    drop(world);
}

#[scenario(
    path = "tests/features/overpass_enrichment.feature",
    name = "Overpass enrichment opens the circuit after repeated failures"
)]
fn overpass_enrichment_opens_the_circuit_after_repeated_failures(world: OverpassEnrichmentWorld) {
    drop(world);
}

#[scenario(
    path = "tests/features/overpass_enrichment.feature",
    name = "Overpass enrichment recovers after circuit cooldown"
)]
fn overpass_enrichment_recovers_after_circuit_cooldown(world: OverpassEnrichmentWorld) {
    drop(world);
}
