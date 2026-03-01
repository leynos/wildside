//! Persistence failure behaviour cases for Overpass enrichment worker tests.

use super::*;

struct PersistenceFailureCase {
    repo_error: OsmPoiRepositoryError,
    expected_code: crate::domain::ErrorCode,
    expected_message_fragment: &'static str,
}

async fn assert_persistence_failure_records_metrics_and_maps_error(
    now: DateTime<Utc>,
    job: OverpassEnrichmentRequest,
    case: PersistenceFailureCase,
) {
    let source = Arc::new(SourceStub::scripted(vec![Ok(response(1, 32))]));
    let repo = Arc::new(RepoStub::new(vec![Err(case.repo_error)]));
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

    let error = worker
        .process_job(job)
        .await
        .expect_err("persistence failures should fail the job");
    assert_eq!(error.code(), case.expected_code);
    assert!(
        error.message().contains(case.expected_message_fragment),
        "error message should contain `{}`",
        case.expected_message_fragment,
    );
    assert_eq!(source.calls.load(Ordering::SeqCst), 1);
    assert_eq!(repo.calls.load(Ordering::SeqCst), 1);
    assert_eq!(provenance_repo.calls.load(Ordering::SeqCst), 0);
    assert!(metrics.successes.lock().expect("metrics mutex").is_empty());
    assert_eq!(metrics.failures.lock().expect("metrics mutex").len(), 1);
    assert_eq!(
        metrics.failures.lock().expect("metrics mutex")[0].kind,
        EnrichmentJobFailureKind::PersistenceFailed
    );
}

#[rstest]
#[tokio::test]
async fn persistence_connection_failure_records_metric_and_maps_to_service_unavailable(
    now: DateTime<Utc>,
    job: OverpassEnrichmentRequest,
) {
    assert_persistence_failure_records_metrics_and_maps_error(
        now,
        job,
        PersistenceFailureCase {
            repo_error: OsmPoiRepositoryError::connection("database down"),
            expected_code: crate::domain::ErrorCode::ServiceUnavailable,
            expected_message_fragment: "unavailable",
        },
    )
    .await;
}

#[rstest]
#[tokio::test]
async fn persistence_query_failure_records_metric_and_maps_to_internal(
    now: DateTime<Utc>,
    job: OverpassEnrichmentRequest,
) {
    assert_persistence_failure_records_metrics_and_maps_error(
        now,
        job,
        PersistenceFailureCase {
            repo_error: OsmPoiRepositoryError::query("write failed"),
            expected_code: crate::domain::ErrorCode::InternalError,
            expected_message_fragment: "failed",
        },
    )
    .await;
}
