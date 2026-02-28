//! Provenance persistence behaviour cases for Overpass enrichment worker tests.

use super::*;

struct ProvenanceFailureCase {
    provenance_error: EnrichmentProvenanceRepositoryError,
    expected_code: crate::domain::ErrorCode,
    expected_message_fragment: &'static str,
}

async fn assert_provenance_failure_records_metrics_and_maps_error(
    now: DateTime<Utc>,
    job: OverpassEnrichmentRequest,
    case: ProvenanceFailureCase,
) {
    let source = Arc::new(SourceStub::scripted(vec![Ok(response(1, 32))]));
    let repo = Arc::new(RepoStub::new(vec![Ok(())]));
    let provenance_repo = Arc::new(ProvenanceRepoStub::new(vec![Err(case.provenance_error)]));
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

    let error = worker
        .process_job(job)
        .await
        .expect_err("provenance failures should fail the job");
    assert_eq!(error.code(), case.expected_code);
    assert!(
        error.message().contains(case.expected_message_fragment),
        "error message should contain `{}`",
        case.expected_message_fragment,
    );
    assert_eq!(source.calls.load(Ordering::SeqCst), 1);
    assert_eq!(repo.calls.load(Ordering::SeqCst), 1);
    assert_eq!(provenance_repo.calls.load(Ordering::SeqCst), 1);
    assert!(metrics.successes.lock().expect("metrics mutex").is_empty());
    assert_eq!(metrics.failures.lock().expect("metrics mutex").len(), 1);
    assert_eq!(
        metrics.failures.lock().expect("metrics mutex")[0].kind,
        EnrichmentJobFailureKind::PersistenceFailed
    );
}

#[rstest]
#[case::connection(
        ProvenanceFailureCase {
        provenance_error: EnrichmentProvenanceRepositoryError::connection("database down"),
        expected_code: crate::domain::ErrorCode::ServiceUnavailable,
        expected_message_fragment: "unavailable",
    }
)]
#[case::query(
    ProvenanceFailureCase {
        provenance_error: EnrichmentProvenanceRepositoryError::query("insert failed"),
        expected_code: crate::domain::ErrorCode::InternalError,
        expected_message_fragment: "failed",
    }
)]
#[tokio::test]
async fn provenance_failure_records_metric_and_maps_error(
    now: DateTime<Utc>,
    job: OverpassEnrichmentRequest,
    #[case] case: ProvenanceFailureCase,
) {
    assert_provenance_failure_records_metrics_and_maps_error(now, job, case).await;
}

#[rstest]
#[tokio::test]
async fn source_url_is_persisted_verbatim(now: DateTime<Utc>, mut job: OverpassEnrichmentRequest) {
    let edge_source_url =
        "https://overpass.example/api/interpreter?mirror=2&query=name%3Dfoo+bar#regional";
    job.bounding_box = [-3.399_999, 55.900_001, -3.100_001, 56.000_001];

    let source = Arc::new(SourceStub::scripted(vec![Ok(response_with_source_url(
        1,
        128,
        edge_source_url,
    ))]));
    let repo = Arc::new(RepoStub::new(vec![Ok(())]));
    let provenance_repo = Arc::new(ProvenanceRepoStub::new(vec![Ok(())]));
    let worker = OverpassEnrichmentWorker::with_runtime(
        OverpassEnrichmentWorkerPorts::new(
            source,
            repo,
            provenance_repo.clone(),
            Arc::new(MetricsStub::default()),
        ),
        Arc::new(MutableClock::new(now)),
        OverpassEnrichmentWorkerRuntime {
            sleeper: Arc::new(RecordingSleeper::default()),
            jitter: Arc::new(NoJitter),
        },
        config(),
    );

    let outcome = worker.process_job(job.clone()).await.expect("job succeeds");
    assert_eq!(outcome.persisted_poi_count, 1);
    let persisted = provenance_repo.persisted.lock().expect("provenance mutex");
    assert_eq!(persisted.len(), 1);
    assert_eq!(persisted[0].source_url, edge_source_url);
    assert_eq!(persisted[0].imported_at, now);
    assert_eq!(persisted[0].bounding_box, job.bounding_box);
}
