//! Provenance persistence behaviour cases for Overpass enrichment worker tests.

use super::*;

struct ProvenanceFailureCase {
    provenance_error: EnrichmentProvenanceRepositoryError,
    expected_code: crate::domain::ErrorCode,
    expected_message_fragment: &'static str,
}

#[fixture]
fn worker_fixture(now: DateTime<Utc>) -> WorkerTestFixture {
    WorkerTestFixtureBuilder::new(now)
        .with_source_responses(vec![Ok(response(1, 32))])
        .with_repo_responses(vec![Ok(())])
        .with_provenance_responses(vec![Ok(())])
        .build()
}

fn set_source_script(
    source: &SourceStub,
    scripted: Vec<Result<OverpassEnrichmentResponse, OverpassEnrichmentSourceError>>,
) {
    *source.scripted.lock().expect("source mutex") = scripted.into();
}

fn set_repo_script(repo: &RepoStub, scripted: Vec<Result<(), OsmPoiRepositoryError>>) {
    *repo.scripted.lock().expect("repo mutex") = scripted.into();
}

fn set_provenance_script(
    provenance_repo: &ProvenanceRepoStub,
    scripted: Vec<Result<(), EnrichmentProvenanceRepositoryError>>,
) {
    *provenance_repo.scripted.lock().expect("provenance mutex") = scripted.into();
}

async fn assert_provenance_failure_records_metrics_and_maps_error(
    fixture: WorkerTestFixture,
    job: OverpassEnrichmentRequest,
    case: ProvenanceFailureCase,
) {
    set_source_script(fixture.source.as_ref(), vec![Ok(response(1, 32))]);
    set_repo_script(fixture.repo.as_ref(), vec![Ok(())]);
    set_provenance_script(
        fixture.provenance_repo.as_ref(),
        vec![Err(case.provenance_error)],
    );

    let error = fixture
        .worker
        .process_job(job)
        .await
        .expect_err("provenance failures should fail the job");
    assert_eq!(error.code(), case.expected_code);
    assert!(
        error.message().contains(case.expected_message_fragment),
        "error message should contain `{}`",
        case.expected_message_fragment,
    );
    assert_eq!(fixture.source.calls.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.repo.calls.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.provenance_repo.calls.load(Ordering::SeqCst), 1);
    assert!(
        fixture
            .metrics
            .successes
            .lock()
            .expect("metrics mutex")
            .is_empty()
    );
    assert_eq!(
        fixture
            .metrics
            .failures
            .lock()
            .expect("metrics mutex")
            .len(),
        1
    );
    assert_eq!(
        fixture.metrics.failures.lock().expect("metrics mutex")[0].kind,
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
    job: OverpassEnrichmentRequest,
    worker_fixture: WorkerTestFixture,
    #[case] case: ProvenanceFailureCase,
) {
    assert_provenance_failure_records_metrics_and_maps_error(worker_fixture, job, case).await;
}

#[rstest]
#[tokio::test]
async fn source_url_is_persisted_verbatim(
    now: DateTime<Utc>,
    mut job: OverpassEnrichmentRequest,
    worker_fixture: WorkerTestFixture,
) {
    let edge_source_url =
        "https://overpass.example/api/interpreter?mirror=2&query=name%3Dfoo+bar#regional";
    job.bounding_box = [-3.399_999, 55.900_001, -3.100_001, 56.000_001];

    set_source_script(
        worker_fixture.source.as_ref(),
        vec![Ok(response_with_source_url(1, 128, edge_source_url))],
    );
    set_repo_script(worker_fixture.repo.as_ref(), vec![Ok(())]);
    set_provenance_script(worker_fixture.provenance_repo.as_ref(), vec![Ok(())]);

    let outcome = worker_fixture
        .worker
        .process_job(job.clone())
        .await
        .expect("job succeeds");
    assert_eq!(outcome.persisted_poi_count, 1);
    let persisted = worker_fixture
        .provenance_repo
        .persisted
        .lock()
        .expect("provenance mutex");
    assert_eq!(persisted.len(), 1);
    assert_eq!(persisted[0].job_id, job.job_id);
    assert_eq!(persisted[0].source_url, edge_source_url);
    assert_eq!(persisted[0].imported_at, now);
    assert_eq!(persisted[0].bounding_box, job.bounding_box);
}
