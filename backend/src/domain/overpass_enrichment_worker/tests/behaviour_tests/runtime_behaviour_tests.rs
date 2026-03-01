//! Runtime-oriented behaviour tests for the Overpass enrichment worker.

use super::*;

#[rstest]
#[tokio::test]
async fn half_open_probe_success_closes_circuit(
    now: DateTime<Utc>,
    job: OverpassEnrichmentRequest,
) {
    let fixture = CircuitBreakerTestFixtureBuilder::new(
        now,
        vec![
            Err(OverpassEnrichmentSourceError::transport("failure")),
            Ok(response(1, 20)),
            Ok(response(1, 20)),
        ],
        1,
        Duration::from_secs(60),
    )
    .build();

    let _ = fixture
        .process_job(job.clone())
        .await
        .expect_err("initial failure");
    fixture.advance_clock(Duration::from_secs(61));
    fixture
        .process_job(job.clone())
        .await
        .expect("probe succeeds");
    fixture
        .process_job(job)
        .await
        .expect("closed circuit allows call");

    assert_eq!(fixture.source.calls.load(Ordering::SeqCst), 3);
    assert_eq!(fixture.circuit_state(), CircuitBreakerState::Closed);
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
            Arc::new(ProvenanceRepoStub::new(vec![Ok(()), Ok(())])),
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
