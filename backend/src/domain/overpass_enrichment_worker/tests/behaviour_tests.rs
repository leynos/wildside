//! Behaviour-focused test cases for the Overpass enrichment worker.

use super::*;

mod persistence_tests;
mod provenance_tests;
mod runtime_behaviour_tests;

fn assert_successful_job_outcome(
    outcome: &crate::domain::OverpassEnrichmentJobOutcome,
    expected_attempts: u32,
    expected_poi_count: usize,
) {
    assert_eq!(
        outcome.attempts, expected_attempts,
        "expected successful job to use {expected_attempts} attempt(s), got {}",
        outcome.attempts
    );
    assert_eq!(
        outcome.persisted_poi_count, expected_poi_count,
        "expected successful job to persist {expected_poi_count} POI(s), got {}",
        outcome.persisted_poi_count
    );
}

#[expect(
    clippy::too_many_arguments,
    reason = "Helper signature is intentionally explicit to mirror the required test assertion contract."
)]
fn assert_stub_call_counts(
    source: &Arc<SourceStub>,
    repo: &Arc<RepoStub>,
    provenance_repo: &Arc<ProvenanceRepoStub>,
    expected_source: usize,
    expected_repo: usize,
    expected_provenance: usize,
) {
    let source_calls = source.calls.load(Ordering::SeqCst);
    let repo_calls = repo.calls.load(Ordering::SeqCst);
    let provenance_calls = provenance_repo.calls.load(Ordering::SeqCst);

    assert_eq!(
        source_calls, expected_source,
        "expected source stub to be called {expected_source} time(s), got {source_calls}"
    );
    assert_eq!(
        repo_calls, expected_repo,
        "expected repository stub to be called {expected_repo} time(s), got {repo_calls}"
    );
    assert_eq!(
        provenance_calls, expected_provenance,
        "expected provenance repository stub to be called {expected_provenance} time(s), got {provenance_calls}"
    );
}

fn assert_provenance_recorded(
    provenance_repo: &ProvenanceRepoStub,
    expected_source_url: &str,
    expected_imported_at: DateTime<Utc>,
    expected_bounding_box: [f64; 4],
) {
    let persisted = provenance_repo.persisted.lock().expect("provenance mutex");
    assert_eq!(
        persisted.len(),
        1,
        "expected exactly one persisted provenance record, got {}",
        persisted.len()
    );

    let record = &persisted[0];
    assert_eq!(
        record.source_url, expected_source_url,
        "expected provenance source URL to be `{expected_source_url}`, got `{}`",
        record.source_url
    );
    assert_eq!(
        record.imported_at, expected_imported_at,
        "expected provenance timestamp to be {expected_imported_at:?}, got {:?}",
        record.imported_at
    );
    assert_eq!(
        record.bounding_box, expected_bounding_box,
        "expected provenance bounding box to be {expected_bounding_box:?}, got {:?}",
        record.bounding_box
    );
}

fn assert_metrics_success(metrics: &MetricsStub, expected_count: usize) {
    let successes = metrics.successes.lock().expect("metrics mutex");
    assert_eq!(
        successes.len(),
        expected_count,
        "expected {expected_count} success metric record(s), got {}",
        successes.len()
    );
    drop(successes);

    let failures = metrics.failures.lock().expect("metrics mutex");
    assert!(
        failures.is_empty(),
        "expected no failure metrics when asserting success, found {}",
        failures.len()
    );
}

fn assert_metrics_failure(metrics: &MetricsStub, expected_failure_kind: EnrichmentJobFailureKind) {
    let successes = metrics.successes.lock().expect("metrics mutex");
    assert!(
        successes.is_empty(),
        "expected no success metrics when asserting failure, found {}",
        successes.len()
    );
    drop(successes);

    let failures = metrics.failures.lock().expect("metrics mutex");
    assert_eq!(
        failures.len(),
        1,
        "expected exactly one failure metric record, got {}",
        failures.len()
    );
    assert_eq!(
        failures[0].kind, expected_failure_kind,
        "expected failure metric kind to be {expected_failure_kind:?}, got {:?}",
        failures[0].kind
    );
}

#[rstest]
#[tokio::test]
async fn happy_path_persists_and_records_success(
    now: DateTime<Utc>,
    job: OverpassEnrichmentRequest,
) {
    let source = Arc::new(SourceStub::scripted(vec![Ok(response(2, 512))]));
    let repo = Arc::new(RepoStub::new(vec![Ok(())]));
    let provenance_repo = Arc::new(ProvenanceRepoStub::new(vec![Ok(())]));
    let metrics = Arc::new(MetricsStub::default());
    let worker = OverpassEnrichmentWorker::with_runtime(
        OverpassEnrichmentWorkerPorts::new(
            source.clone(),
            repo.clone(),
            provenance_repo.clone(),
            metrics.clone(),
        ),
        Arc::new(MutableClock::new(now)),
        OverpassEnrichmentWorkerRuntime {
            sleeper: Arc::new(RecordingSleeper::default()),
            jitter: Arc::new(NoJitter),
        },
        config(),
    );

    let out = worker.process_job(job.clone()).await.expect("job succeeds");
    assert_successful_job_outcome(&out, 1, 2);
    assert_stub_call_counts(&source, &repo, &provenance_repo, 1, 1, 1);
    assert_provenance_recorded(
        provenance_repo.as_ref(),
        "https://overpass.example/api/interpreter",
        now,
        job.bounding_box,
    );
    assert_metrics_success(metrics.as_ref(), 1);
}

#[derive(Debug, Clone, Copy)]
struct QuotaCase {
    max_requests: u32,
    max_transfer_bytes: u64,
    expected_kind: EnrichmentJobFailureKind,
}

async fn assert_quota_denial_short_circuits_source(
    now: DateTime<Utc>,
    job: OverpassEnrichmentRequest,
    case: QuotaCase,
) {
    let source = Arc::new(SourceStub::scripted(vec![Ok(response(1, 10))]));
    let repo = Arc::new(RepoStub::new(vec![Ok(())]));
    let provenance_repo = Arc::new(ProvenanceRepoStub::new(vec![]));
    let mut cfg = config();
    cfg.max_daily_requests = case.max_requests;
    cfg.max_daily_transfer_bytes = case.max_transfer_bytes;
    let metrics = Arc::new(MetricsStub::default());
    let worker = OverpassEnrichmentWorker::with_runtime(
        OverpassEnrichmentWorkerPorts::new(
            source.clone(),
            repo.clone(),
            provenance_repo.clone(),
            metrics.clone(),
        ),
        Arc::new(MutableClock::new(now)),
        OverpassEnrichmentWorkerRuntime {
            sleeper: Arc::new(RecordingSleeper::default()),
            jitter: Arc::new(NoJitter),
        },
        cfg,
    );

    let error = worker.process_job(job).await.expect_err("quota denies");
    assert_eq!(error.code(), crate::domain::ErrorCode::ServiceUnavailable);
    assert_stub_call_counts(&source, &repo, &provenance_repo, 0, 0, 0);
    assert_metrics_failure(metrics.as_ref(), case.expected_kind);
}

#[rstest]
#[case::request_limit(QuotaCase {
    max_requests: 0,
    max_transfer_bytes: 10,
    expected_kind: EnrichmentJobFailureKind::QuotaRequestLimit,
})]
#[case::transfer_limit(QuotaCase {
    max_requests: 10,
    max_transfer_bytes: 0,
    expected_kind: EnrichmentJobFailureKind::QuotaTransferLimit,
})]
#[tokio::test]
async fn quota_limit_denial_short_circuits_source(
    now: DateTime<Utc>,
    job: OverpassEnrichmentRequest,
    #[case] case: QuotaCase,
) {
    assert_quota_denial_short_circuits_source(now, job, case).await;
}

#[rstest]
#[tokio::test]
async fn retry_uses_jittered_exponential_backoff(
    now: DateTime<Utc>,
    job: OverpassEnrichmentRequest,
) {
    let source = Arc::new(SourceStub::scripted(vec![
        Err(OverpassEnrichmentSourceError::transport(
            "temporary transport",
        )),
        Err(OverpassEnrichmentSourceError::timeout("temporary timeout")),
        Ok(response(1, 64)),
    ]));
    let repo = Arc::new(RepoStub::new(vec![Ok(())]));
    let sleeper = Arc::new(RecordingSleeper::default());
    let provenance_repo = Arc::new(ProvenanceRepoStub::new(vec![Ok(())]));
    let metrics = Arc::new(MetricsStub::default());
    let worker = OverpassEnrichmentWorker::with_runtime(
        OverpassEnrichmentWorkerPorts::new(
            source.clone(),
            repo.clone(),
            provenance_repo.clone(),
            metrics.clone(),
        ),
        Arc::new(MutableClock::new(now)),
        OverpassEnrichmentWorkerRuntime {
            sleeper: sleeper.clone(),
            jitter: Arc::new(AttemptOffsetJitter),
        },
        config(),
    );

    let out = worker.process_job(job).await.expect("eventual success");
    assert_successful_job_outcome(&out, 3, 1);
    assert_stub_call_counts(&source, &repo, &provenance_repo, 3, 1, 1);
    assert_eq!(
        sleeper.lock_durations().as_slice(),
        [Duration::from_millis(101), Duration::from_millis(202)]
    );
    assert_metrics_success(metrics.as_ref(), 1);
    assert_eq!(
        metrics.successes.lock().expect("metrics mutex")[0].attempt_count,
        3
    );
}

struct CircuitBreakerTestFixtureBuilder {
    now: DateTime<Utc>,
    source_responses: Vec<Result<OverpassEnrichmentResponse, OverpassEnrichmentSourceError>>,
    failure_threshold: u32,
    cooldown_duration: Duration,
}

impl CircuitBreakerTestFixtureBuilder {
    fn new(
        now: DateTime<Utc>,
        source_responses: Vec<Result<OverpassEnrichmentResponse, OverpassEnrichmentSourceError>>,
        failure_threshold: u32,
        cooldown_duration: Duration,
    ) -> Self {
        Self {
            now,
            source_responses,
            failure_threshold,
            cooldown_duration,
        }
    }

    fn build(self) -> CircuitBreakerTestFixture {
        let clock = Arc::new(MutableClock::new(self.now));
        let source = Arc::new(SourceStub::scripted(self.source_responses));
        let repo = Arc::new(RepoStub::new(vec![Ok(()), Ok(())]));
        let provenance_repo = Arc::new(ProvenanceRepoStub::new(vec![Ok(()), Ok(())]));

        let mut cfg = config();
        cfg.max_attempts = 1;
        cfg.circuit_failure_threshold = self.failure_threshold;
        cfg.circuit_open_cooldown = self.cooldown_duration;

        let worker = OverpassEnrichmentWorker::with_runtime(
            OverpassEnrichmentWorkerPorts::new(
                source.clone(),
                repo.clone(),
                provenance_repo.clone(),
                Arc::new(MetricsStub::default()),
            ),
            clock.clone(),
            OverpassEnrichmentWorkerRuntime {
                sleeper: Arc::new(RecordingSleeper::default()),
                jitter: Arc::new(NoJitter),
            },
            cfg,
        );

        CircuitBreakerTestFixture {
            worker,
            source,
            repo,
            provenance_repo,
            clock,
        }
    }
}

struct CircuitBreakerTestFixture {
    worker: OverpassEnrichmentWorker,
    source: Arc<SourceStub>,
    repo: Arc<RepoStub>,
    provenance_repo: Arc<ProvenanceRepoStub>,
    clock: Arc<MutableClock>,
}

impl CircuitBreakerTestFixture {
    async fn process_job(
        &self,
        request: OverpassEnrichmentRequest,
    ) -> Result<crate::domain::OverpassEnrichmentJobOutcome, crate::domain::Error> {
        self.worker.process_job(request).await
    }

    fn advance_clock(&self, delta: Duration) {
        self.clock.advance(delta);
    }

    fn circuit_state(&self) -> CircuitBreakerState {
        self.worker
            .policy_state
            .lock()
            .expect("policy mutex")
            .circuit_state()
    }
}

#[rstest]
#[tokio::test]
async fn circuit_opens_and_blocks_until_cooldown(
    now: DateTime<Utc>,
    job: OverpassEnrichmentRequest,
) {
    let fixture = CircuitBreakerTestFixtureBuilder::new(
        now,
        vec![
            Err(OverpassEnrichmentSourceError::transport("failure-1")),
            Err(OverpassEnrichmentSourceError::transport("failure-2")),
        ],
        2,
        Duration::from_secs(120),
    )
    .build();

    let _ = fixture
        .process_job(job.clone())
        .await
        .expect_err("first fails");
    let _ = fixture
        .process_job(job.clone())
        .await
        .expect_err("second fails");
    let blocked = fixture
        .process_job(job)
        .await
        .expect_err("blocked by open circuit");

    assert_eq!(blocked.code(), crate::domain::ErrorCode::ServiceUnavailable);
    assert_stub_call_counts(
        &fixture.source,
        &fixture.repo,
        &fixture.provenance_repo,
        2,
        0,
        0,
    );
    assert_eq!(fixture.circuit_state(), CircuitBreakerState::Open);
}
