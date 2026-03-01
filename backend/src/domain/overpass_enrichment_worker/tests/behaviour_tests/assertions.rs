//! Assertion helpers for Overpass enrichment worker behaviour tests.

use super::*;

pub(super) fn assert_successful_job_outcome(
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

#[derive(Debug, Clone, Copy)]
pub(super) struct StubCallCountExpectations {
    pub(super) source: usize,
    pub(super) repository: usize,
    pub(super) provenance_repository: usize,
}

pub(super) struct StubCallCounters<'a> {
    pub(super) source: &'a SourceStub,
    pub(super) repository: &'a RepoStub,
    pub(super) provenance_repository: &'a ProvenanceRepoStub,
}

pub(super) fn assert_stub_call_counts(
    counters: StubCallCounters<'_>,
    expected: StubCallCountExpectations,
) {
    let source_calls = counters.source.calls.load(Ordering::SeqCst);
    let repo_calls = counters.repository.calls.load(Ordering::SeqCst);
    let provenance_calls = counters.provenance_repository.calls.load(Ordering::SeqCst);

    assert_eq!(
        source_calls, expected.source,
        "expected source stub to be called {} time(s), got {source_calls}",
        expected.source
    );
    assert_eq!(
        repo_calls, expected.repository,
        "expected repository stub to be called {} time(s), got {repo_calls}",
        expected.repository
    );
    assert_eq!(
        provenance_calls, expected.provenance_repository,
        "expected provenance repository stub to be called {} time(s), got {provenance_calls}",
        expected.provenance_repository
    );
}

pub(super) fn assert_provenance_recorded(
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

pub(super) fn assert_metrics_success(metrics: &MetricsStub, expected_count: usize) {
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

pub(super) fn assert_metrics_failure(
    metrics: &MetricsStub,
    expected_failure_kind: EnrichmentJobFailureKind,
) {
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
