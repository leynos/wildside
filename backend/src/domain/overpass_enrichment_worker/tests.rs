//! Unit tests for Overpass enrichment worker orchestration.

use std::collections::{BTreeMap, VecDeque};
use std::error::Error as StdError;
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use rstest::{fixture, rstest};
use tokio::sync::{Notify, mpsc};
use tokio::time::timeout;
use uuid::Uuid;

use super::policy::CircuitBreakerState;
use super::{
    OverpassEnrichmentWorker, OverpassEnrichmentWorkerConfig, OverpassEnrichmentWorkerPorts,
    OverpassEnrichmentWorkerRuntime,
};
use crate::domain::BoundingBox;
use crate::domain::ports::{
    EnrichmentJobFailure, EnrichmentJobFailureKind, EnrichmentJobMetrics,
    EnrichmentJobMetricsError, EnrichmentJobSuccess, EnrichmentPoi, EnrichmentProvenanceRecord,
    EnrichmentProvenanceRepository, EnrichmentProvenanceRepositoryError, EnrichmentRequest,
    EnrichmentResponse, EnrichmentSource, EnrichmentSourceError, ListEnrichmentProvenanceRequest,
    ListEnrichmentProvenanceResponse, OsmPoiIngestionRecord, OsmPoiRepository,
    OsmPoiRepositoryError,
};
use crate::test_support::overpass_enrichment::{
    AttemptOffsetJitter, MutableClock, NoJitter, RecordingSleeper,
};

type TestResult<T = ()> = Result<T, Box<dyn StdError>>;

struct SourceStub {
    scripted: Mutex<VecDeque<Result<EnrichmentResponse, EnrichmentSourceError>>>,
    calls: AtomicUsize,
    active: AtomicUsize,
    max_active: AtomicUsize,
    entered: Option<mpsc::UnboundedSender<usize>>,
    release: Option<Arc<Notify>>,
}
impl SourceStub {
    fn scripted(scripted: Vec<Result<EnrichmentResponse, EnrichmentSourceError>>) -> Self {
        Self {
            scripted: Mutex::new(scripted.into()),
            calls: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            max_active: AtomicUsize::new(0),
            entered: None,
            release: None,
        }
    }
    fn blocking(
        scripted: Vec<Result<EnrichmentResponse, EnrichmentSourceError>>,
        entered: mpsc::UnboundedSender<usize>,
        release: Arc<Notify>,
    ) -> Self {
        Self {
            entered: Some(entered),
            release: Some(release),
            ..Self::scripted(scripted)
        }
    }
}
#[async_trait]
impl EnrichmentSource for SourceStub {
    async fn fetch_pois(
        &self,
        _request: &EnrichmentRequest,
    ) -> Result<EnrichmentResponse, EnrichmentSourceError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let active_now = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_active.fetch_max(active_now, Ordering::SeqCst);
        if let Some(entered) = &self.entered {
            entered
                .send(active_now)
                .map_err(|error| EnrichmentSourceError::invalid_request(error.to_string()))?;
        }
        if let Some(release) = &self.release {
            release.notified().await;
        }
        self.active.fetch_sub(1, Ordering::SeqCst);
        self.scripted
            .lock()
            .map_err(|_| EnrichmentSourceError::invalid_request("source mutex"))?
            .pop_front()
            .unwrap_or_else(|| {
                Err(EnrichmentSourceError::invalid_request(
                    "source script exhausted unexpectedly",
                ))
            })
    }
}

trait ScriptedRepositoryRecord {
    const MUTEX_MESSAGE: &'static str;
}

trait ScriptedRepositoryError {
    fn mutex_error(message: &str) -> Self;
}

impl ScriptedRepositoryRecord for Vec<OsmPoiIngestionRecord> {
    const MUTEX_MESSAGE: &'static str = "repo mutex";
}

impl ScriptedRepositoryRecord for EnrichmentProvenanceRecord {
    const MUTEX_MESSAGE: &'static str = "provenance mutex";
}

impl ScriptedRepositoryError for OsmPoiRepositoryError {
    fn mutex_error(message: &str) -> Self {
        Self::query(message)
    }
}

impl ScriptedRepositoryError for EnrichmentProvenanceRepositoryError {
    fn mutex_error(message: &str) -> Self {
        Self::query(message)
    }
}

struct ScriptedRepositoryStub<T, E> {
    scripted: Mutex<VecDeque<Result<(), E>>>,
    calls: AtomicUsize,
    persisted: Mutex<Vec<T>>,
}

impl<T, E> ScriptedRepositoryStub<T, E>
where
    T: Clone + ScriptedRepositoryRecord,
    E: ScriptedRepositoryError,
{
    fn new(scripted: Vec<Result<(), E>>) -> Self {
        Self {
            scripted: Mutex::new(scripted.into()),
            calls: AtomicUsize::new(0),
            persisted: Mutex::new(Vec::new()),
        }
    }

    fn persist_internal(&self, record: &T) -> Result<(), E> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut scripted = self
            .scripted
            .lock()
            .map_err(|_| E::mutex_error(T::MUTEX_MESSAGE))?;
        match scripted.pop_front() {
            Some(Ok(())) => {
                drop(scripted);
                self.persisted
                    .lock()
                    .map_err(|_| E::mutex_error(T::MUTEX_MESSAGE))?
                    .push(record.clone());
                Ok(())
            }
            Some(Err(error)) => Err(error),
            None => {
                debug_assert!(
                    false,
                    "ScriptedRepositoryStub: no scripted responses left for {}",
                    std::any::type_name::<T>(),
                );
                Ok(())
            }
        }
    }
}

type RepoStub = ScriptedRepositoryStub<Vec<OsmPoiIngestionRecord>, OsmPoiRepositoryError>;

#[async_trait]
impl OsmPoiRepository for RepoStub {
    async fn upsert_pois(
        &self,
        records: &[OsmPoiIngestionRecord],
    ) -> Result<(), OsmPoiRepositoryError> {
        self.persist_internal(&records.to_vec())
    }
}

type ProvenanceRepoStub =
    ScriptedRepositoryStub<EnrichmentProvenanceRecord, EnrichmentProvenanceRepositoryError>;

#[async_trait]
impl EnrichmentProvenanceRepository for ProvenanceRepoStub {
    async fn persist(
        &self,
        record: &EnrichmentProvenanceRecord,
    ) -> Result<(), EnrichmentProvenanceRepositoryError> {
        self.persist_internal(record)
    }

    async fn list_recent(
        &self,
        _request: &ListEnrichmentProvenanceRequest,
    ) -> Result<ListEnrichmentProvenanceResponse, EnrichmentProvenanceRepositoryError> {
        Ok(ListEnrichmentProvenanceResponse {
            records: Vec::new(),
            next_before: None,
        })
    }
}

#[derive(Default)]
struct MetricsStub {
    successes: Mutex<Vec<EnrichmentJobSuccess>>,
    failures: Mutex<Vec<EnrichmentJobFailure>>,
}

impl MetricsStub {
    fn record<T: Clone>(
        &self,
        entries: &Mutex<Vec<T>>,
        payload: &T,
    ) -> Result<(), EnrichmentJobMetricsError> {
        entries
            .lock()
            .map_err(|_| EnrichmentJobMetricsError::export("metrics mutex"))?
            .push(payload.clone());
        Ok(())
    }
}

#[async_trait]
impl EnrichmentJobMetrics for MetricsStub {
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

#[fixture]
fn now() -> TestResult<DateTime<Utc>> {
    Ok(Utc
        .with_ymd_and_hms(2026, 2, 26, 12, 0, 0)
        .single()
        .ok_or_else(|| io::Error::other("valid time"))?)
}
#[fixture]
fn job() -> EnrichmentRequest {
    EnrichmentRequest {
        job_id: Uuid::new_v4(),
        bounding_box: test_bounding_box(-3.30, 55.90, -3.10, 56.00),
        tags: vec!["amenity".to_owned()],
    }
}

/// Build a bounding box for tests, panicking on coordinates that cannot be one.
fn test_bounding_box(min_lng: f64, min_lat: f64, max_lng: f64, max_lat: f64) -> BoundingBox {
    match BoundingBox::new(min_lng, min_lat, max_lng, max_lat) {
        Ok(bounding_box) => bounding_box,
        Err(error) => panic!("test bounding box should be valid: {error}"),
    }
}

fn config() -> OverpassEnrichmentWorkerConfig {
    OverpassEnrichmentWorkerConfig {
        max_concurrent_calls: 2,
        max_daily_requests: 10_000,
        max_daily_transfer_bytes: 1_073_741_824,
        max_attempts: 3,
        initial_backoff: Duration::from_millis(100),
        max_backoff: Duration::from_millis(500),
        circuit_failure_threshold: 3,
        circuit_open_cooldown: Duration::from_secs(60),
    }
}

fn response(poi_count: usize, transfer_bytes: u64) -> EnrichmentResponse {
    response_with_source_url(
        poi_count,
        transfer_bytes,
        "https://overpass.example/api/interpreter",
    )
}

fn response_with_source_url(
    poi_count: usize,
    transfer_bytes: u64,
    source_url: &str,
) -> EnrichmentResponse {
    EnrichmentResponse {
        transfer_bytes,
        source_url: source_url.to_owned(),
        pois: (0..poi_count)
            .map(|idx| EnrichmentPoi {
                element_type: "node".to_owned(),
                element_id: idx as i64,
                longitude: -3.2,
                latitude: 55.9,
                tags: BTreeMap::from([("name".to_owned(), format!("poi-{idx}"))]),
            })
            .collect(),
    }
}

mod behaviour_tests;
