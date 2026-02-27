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
    BackoffJitter, EnrichmentSleeper, OverpassEnrichmentJobOutcome, OverpassEnrichmentWorker,
};
use chrono::{DateTime, Local, TimeDelta, Utc};
use mockable::Clock;
use pg_embedded_setup_unpriv::TemporaryDatabase;
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;
use std::time::Duration;
use tokio::sync::{Mutex as AsyncMutex, Notify, mpsc};

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
    active: AtomicUsize,
    max_active: AtomicUsize,
    blocking: Mutex<Option<BlockingControl>>,
}

#[derive(Clone)]
struct BlockingControl {
    entered: mpsc::UnboundedSender<usize>,
    release: Arc<Notify>,
}

impl ScriptedOverpassSource {
    fn new(
        scripted: Vec<Result<OverpassEnrichmentResponse, OverpassEnrichmentSourceError>>,
    ) -> Self {
        Self {
            scripted: Mutex::new(scripted.into()),
            calls: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            max_active: AtomicUsize::new(0),
            blocking: Mutex::new(None),
        }
    }

    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    fn max_active_count(&self) -> usize {
        self.max_active.load(Ordering::SeqCst)
    }

    fn enable_blocking(&self) -> (mpsc::UnboundedReceiver<usize>, Arc<Notify>) {
        let (entered_tx, entered_rx) = mpsc::unbounded_channel();
        let release = Arc::new(Notify::new());
        self.blocking
            .lock()
            .expect("blocking mutex")
            .replace(BlockingControl {
                entered: entered_tx,
                release: release.clone(),
            });
        (entered_rx, release)
    }
}

#[async_trait]
impl OverpassEnrichmentSource for ScriptedOverpassSource {
    async fn fetch_pois(
        &self,
        _request: &OverpassEnrichmentRequest,
    ) -> Result<OverpassEnrichmentResponse, OverpassEnrichmentSourceError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let active_now = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_active.fetch_max(active_now, Ordering::SeqCst);

        let blocking = self.blocking.lock().expect("blocking mutex").clone();
        if let Some(BlockingControl { entered, release }) = blocking {
            entered.send(active_now).expect("send entry");
            release.notified().await;
        }

        self.active.fetch_sub(1, Ordering::SeqCst);
        self.scripted
            .lock()
            .expect("source script mutex")
            .pop_front()
            .unwrap_or_else(|| {
                Err(OverpassEnrichmentSourceError::invalid_request(
                    "source script exhausted unexpectedly",
                ))
            })
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

    fn record<T: Clone>(
        &self,
        entries: &Mutex<Vec<T>>,
        payload: &T,
    ) -> Result<(), EnrichmentJobMetricsError> {
        entries.lock().expect("metrics mutex").push(payload.clone());
        Ok(())
    }
}

#[async_trait]
impl EnrichmentJobMetrics for RecordingEnrichmentMetrics {
    async fn record_success(
        &self,
        payload: &EnrichmentJobSuccess,
    ) -> Result<(), EnrichmentJobMetricsError> {
        self.record(&self.successes, payload)
    }

    async fn record_failure(
        &self,
        payload: &EnrichmentJobFailure,
    ) -> Result<(), EnrichmentJobMetricsError> {
        self.record(&self.failures, payload)
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
    entered_rx: Slot<Arc<AsyncMutex<mpsc::UnboundedReceiver<usize>>>>,
    release_notify: Slot<Arc<Notify>>,
    concurrent_results: Slot<Vec<Result<OverpassEnrichmentJobOutcome, backend::domain::Error>>>,
}

#[fixture]
fn world() -> OverpassEnrichmentWorld {
    OverpassEnrichmentWorld::default()
}
#[path = "overpass_enrichment_bdd/steps.rs"]
mod overpass_enrichment_steps;
