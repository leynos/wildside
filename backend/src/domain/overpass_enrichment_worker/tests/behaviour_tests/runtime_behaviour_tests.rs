//! Runtime-oriented behaviour tests for the Overpass enrichment worker.

use super::*;

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
        let worker_fixture = WorkerTestFixtureBuilder::new(self.now)
            .with_source_responses(self.source_responses)
            .with_repo_responses(vec![Ok(()), Ok(())])
            .with_provenance_responses(vec![Ok(()), Ok(())])
            .with_config({
                let failure_threshold = self.failure_threshold;
                let cooldown_duration = self.cooldown_duration;
                move |cfg| {
                    cfg.max_attempts = 1;
                    cfg.circuit_failure_threshold = failure_threshold;
                    cfg.circuit_open_cooldown = cooldown_duration;
                }
            })
            .build();

        CircuitBreakerTestFixture { worker_fixture }
    }
}

struct CircuitBreakerTestFixture {
    worker_fixture: WorkerTestFixture,
}

impl CircuitBreakerTestFixture {
    async fn process_job(
        &self,
        request: OverpassEnrichmentRequest,
    ) -> Result<crate::domain::OverpassEnrichmentJobOutcome, crate::domain::Error> {
        self.worker_fixture.worker.process_job(request).await
    }

    fn advance_clock(&self, delta: Duration) {
        self.worker_fixture.advance_clock(delta);
    }

    fn circuit_state(&self) -> TestResult<CircuitBreakerState> {
        self.worker_fixture.circuit_state()
    }

    fn stub_call_counters(&self) -> StubCallCounters<'_> {
        StubCallCounters {
            source: self.worker_fixture.source.as_ref(),
            repository: self.worker_fixture.repo.as_ref(),
            provenance_repository: self.worker_fixture.provenance_repo.as_ref(),
        }
    }
}

#[rstest]
#[tokio::test]
async fn half_open_probe_success_closes_circuit(job: OverpassEnrichmentRequest) -> TestResult {
    let fixture = CircuitBreakerTestFixtureBuilder::new(
        now()?,
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
    fixture.process_job(job.clone()).await?;
    fixture.process_job(job).await?;

    assert_eq!(
        fixture
            .stub_call_counters()
            .source
            .calls
            .load(Ordering::SeqCst),
        3
    );
    assert_eq!(fixture.circuit_state()?, CircuitBreakerState::Closed);
    Ok(())
}

#[rstest]
#[tokio::test]
async fn semaphore_limits_concurrent_calls(job: OverpassEnrichmentRequest) -> TestResult {
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
    let fixture = Arc::new(
        WorkerTestFixtureBuilder::new(now()?)
            .with_source(source.clone())
            .with_repo_responses(vec![Ok(()), Ok(())])
            .with_provenance_responses(vec![Ok(()), Ok(())])
            .with_config(|cfg| {
                cfg.max_attempts = 1;
                cfg.max_concurrent_calls = 1;
            })
            .build(),
    );

    let first_fixture = Arc::clone(&fixture);
    let first_job = job.clone();
    let first = tokio::spawn(async move { first_fixture.process_job(first_job).await });
    timeout(Duration::from_secs(1), entered_rx.recv())
        .await
        .map_err(|_| std::io::Error::other("first entered"))?
        .ok_or_else(|| std::io::Error::other("recv() returned None"))?;

    let second_fixture = Arc::clone(&fixture);
    let second = tokio::spawn(async move { second_fixture.process_job(job).await });
    assert!(
        timeout(Duration::from_millis(80), entered_rx.recv())
            .await
            .is_err()
    );

    release.notify_one();
    timeout(Duration::from_secs(1), entered_rx.recv())
        .await
        .map_err(|_| std::io::Error::other("second entered"))?
        .ok_or_else(|| std::io::Error::other("recv() returned None"))?;
    release.notify_one();

    first.await??;
    second.await??;
    assert_eq!(source.max_active.load(Ordering::SeqCst), 1);
    Ok(())
}

#[rstest]
#[tokio::test]
async fn circuit_opens_and_blocks_until_cooldown(job: OverpassEnrichmentRequest) -> TestResult {
    let fixture = CircuitBreakerTestFixtureBuilder::new(
        now()?,
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
        fixture.stub_call_counters(),
        StubCallCountExpectations {
            source: 2,
            repository: 0,
            provenance_repository: 0,
        },
    );
    assert_eq!(fixture.circuit_state()?, CircuitBreakerState::Open);
    Ok(())
}
