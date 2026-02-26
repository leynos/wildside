//! Behaviour-focused test cases for the Overpass enrichment worker.

use super::*;
#[rstest]
#[tokio::test]
async fn happy_path_persists_and_records_success(
    now: DateTime<Utc>,
    job: OverpassEnrichmentRequest,
) {
    let source = Arc::new(SourceStub::scripted(vec![Ok(response(2, 512))]));
    let repo = Arc::new(RepoStub::new(vec![Ok(())]));
    let metrics = Arc::new(MetricsStub::default());
    let worker = OverpassEnrichmentWorker::with_runtime(
        OverpassEnrichmentWorkerPorts::new(source.clone(), repo.clone(), metrics.clone()),
        Arc::new(MutableClock::new(now)),
        OverpassEnrichmentWorkerRuntime {
            sleeper: Arc::new(RecordingSleeper::default()),
            jitter: Arc::new(NoJitter),
        },
        config(),
    );

    let out = worker.process_job(job).await.expect("job succeeds");
    assert_eq!(out.attempts, 1);
    assert_eq!(out.persisted_poi_count, 2);
    assert_eq!(source.calls.load(Ordering::SeqCst), 1);
    assert_eq!(repo.calls.load(Ordering::SeqCst), 1);
    assert_eq!(metrics.successes.lock().expect("metrics mutex").len(), 1);
    assert!(metrics.failures.lock().expect("metrics mutex").is_empty());
}

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
    let mut cfg = config();
    cfg.max_daily_requests = case.max_requests;
    cfg.max_daily_transfer_bytes = case.max_transfer_bytes;
    let metrics = Arc::new(MetricsStub::default());
    let worker = OverpassEnrichmentWorker::with_runtime(
        OverpassEnrichmentWorkerPorts::new(
            source.clone(),
            Arc::new(RepoStub::new(vec![Ok(())])),
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
    assert_eq!(source.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        metrics.failures.lock().expect("metrics mutex")[0].kind,
        case.expected_kind
    );
}

#[rstest]
#[tokio::test]
async fn quota_request_limit_denial_short_circuits_source(
    now: DateTime<Utc>,
    job: OverpassEnrichmentRequest,
) {
    assert_quota_denial_short_circuits_source(
        now,
        job,
        QuotaCase {
            max_requests: 0,
            max_transfer_bytes: 10,
            expected_kind: EnrichmentJobFailureKind::QuotaRequestLimit,
        },
    )
    .await;
}

#[rstest]
#[tokio::test]
async fn quota_transfer_limit_denial_short_circuits_source(
    now: DateTime<Utc>,
    job: OverpassEnrichmentRequest,
) {
    assert_quota_denial_short_circuits_source(
        now,
        job,
        QuotaCase {
            max_requests: 10,
            max_transfer_bytes: 0,
            expected_kind: EnrichmentJobFailureKind::QuotaTransferLimit,
        },
    )
    .await;
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
    let sleeper = Arc::new(RecordingSleeper::default());
    let metrics = Arc::new(MetricsStub::default());
    let worker = OverpassEnrichmentWorker::with_runtime(
        OverpassEnrichmentWorkerPorts::new(
            source.clone(),
            Arc::new(RepoStub::new(vec![Ok(())])),
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
    assert_eq!(out.attempts, 3);
    assert_eq!(source.calls.load(Ordering::SeqCst), 3);
    assert_eq!(
        sleeper.0.lock().expect("sleeper mutex").as_slice(),
        [Duration::from_millis(101), Duration::from_millis(202)]
    );
    assert_eq!(
        metrics.successes.lock().expect("metrics mutex")[0].attempt_count,
        3
    );
}

#[rstest]
#[tokio::test]
async fn circuit_opens_and_blocks_until_cooldown(
    now: DateTime<Utc>,
    job: OverpassEnrichmentRequest,
) {
    let source = Arc::new(SourceStub::scripted(vec![
        Err(OverpassEnrichmentSourceError::transport("failure-1")),
        Err(OverpassEnrichmentSourceError::transport("failure-2")),
    ]));
    let mut cfg = config();
    cfg.max_attempts = 1;
    cfg.circuit_failure_threshold = 2;
    cfg.circuit_open_cooldown = Duration::from_secs(120);
    let worker = OverpassEnrichmentWorker::with_runtime(
        OverpassEnrichmentWorkerPorts::new(
            source.clone(),
            Arc::new(RepoStub::new(vec![Ok(())])),
            Arc::new(MetricsStub::default()),
        ),
        Arc::new(MutableClock::new(now)),
        OverpassEnrichmentWorkerRuntime {
            sleeper: Arc::new(RecordingSleeper::default()),
            jitter: Arc::new(NoJitter),
        },
        cfg,
    );

    let _ = worker
        .process_job(job.clone())
        .await
        .expect_err("first fails");
    let _ = worker
        .process_job(job.clone())
        .await
        .expect_err("second fails");
    let blocked = worker
        .process_job(job)
        .await
        .expect_err("blocked by open circuit");

    assert_eq!(blocked.code(), crate::domain::ErrorCode::ServiceUnavailable);
    assert_eq!(source.calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        worker
            .policy_state
            .lock()
            .expect("policy mutex")
            .circuit_state(),
        CircuitBreakerState::Open
    );
}

#[rstest]
#[tokio::test]
async fn half_open_probe_success_closes_circuit(
    now: DateTime<Utc>,
    job: OverpassEnrichmentRequest,
) {
    let clock = Arc::new(MutableClock::new(now));
    let source = Arc::new(SourceStub::scripted(vec![
        Err(OverpassEnrichmentSourceError::transport("failure")),
        Ok(response(1, 20)),
        Ok(response(1, 20)),
    ]));
    let mut cfg = config();
    cfg.max_attempts = 1;
    cfg.circuit_failure_threshold = 1;
    cfg.circuit_open_cooldown = Duration::from_secs(60);
    let worker = OverpassEnrichmentWorker::with_runtime(
        OverpassEnrichmentWorkerPorts::new(
            source.clone(),
            Arc::new(RepoStub::new(vec![Ok(()), Ok(())])),
            Arc::new(MetricsStub::default()),
        ),
        clock.clone(),
        OverpassEnrichmentWorkerRuntime {
            sleeper: Arc::new(RecordingSleeper::default()),
            jitter: Arc::new(NoJitter),
        },
        cfg,
    );

    let _ = worker
        .process_job(job.clone())
        .await
        .expect_err("initial failure");
    clock.advance(Duration::from_secs(61));
    worker
        .process_job(job.clone())
        .await
        .expect("probe succeeds");
    worker
        .process_job(job)
        .await
        .expect("closed circuit allows call");

    assert_eq!(source.calls.load(Ordering::SeqCst), 3);
    assert_eq!(
        worker
            .policy_state
            .lock()
            .expect("policy mutex")
            .circuit_state(),
        CircuitBreakerState::Closed,
    );
}

#[rstest]
#[tokio::test]
async fn semaphore_limits_concurrent_calls(now: DateTime<Utc>, job: OverpassEnrichmentRequest) {
    let (entered_tx, mut entered_rx) = mpsc::unbounded_channel();
    let release = Arc::new(Notify::new());
    let source = Arc::new(SourceStub::blocking(
        vec![
            Ok(OverpassEnrichmentResponse::default()),
            Ok(OverpassEnrichmentResponse::default()),
        ],
        entered_tx,
        release.clone(),
    ));
    let mut cfg = config();
    cfg.max_attempts = 1;
    cfg.max_concurrent_calls = 1;
    let worker = Arc::new(OverpassEnrichmentWorker::with_runtime(
        OverpassEnrichmentWorkerPorts::new(
            source.clone(),
            Arc::new(RepoStub::new(vec![Ok(()), Ok(())])),
            Arc::new(MetricsStub::default()),
        ),
        Arc::new(MutableClock::new(now)),
        OverpassEnrichmentWorkerRuntime {
            sleeper: Arc::new(RecordingSleeper::default()),
            jitter: Arc::new(NoJitter),
        },
        cfg,
    ));

    let first_worker = Arc::clone(&worker);
    let first_job = job.clone();
    let first = tokio::spawn(async move { first_worker.process_job(first_job).await });
    timeout(Duration::from_secs(1), entered_rx.recv())
        .await
        .expect("first entered")
        .expect("entry exists");

    let second_worker = Arc::clone(&worker);
    let second = tokio::spawn(async move { second_worker.process_job(job).await });
    assert!(
        timeout(Duration::from_millis(80), entered_rx.recv())
            .await
            .is_err()
    );

    release.notify_one();
    timeout(Duration::from_secs(1), entered_rx.recv())
        .await
        .expect("second entered")
        .expect("entry exists");
    release.notify_one();

    first.await.expect("first join").expect("first succeeds");
    second.await.expect("second join").expect("second succeeds");
    assert_eq!(source.max_active.load(Ordering::SeqCst), 1);
}
