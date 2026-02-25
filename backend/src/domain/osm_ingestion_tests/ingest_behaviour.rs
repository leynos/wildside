//! Behavioural unit coverage for OSM ingest orchestration flow.

use mockall::Sequence;
use rstest::rstest;

use super::*;
use crate::domain::ErrorCode;
use crate::domain::ports::{
    OsmIngestionCommand, OsmIngestionProvenanceRepositoryError, OsmIngestionStatus,
    OsmSourceReport, OsmSourceRepositoryError,
};

fn fixture_source_report_for_geofence_filtering() -> OsmSourceReport {
    OsmSourceReport {
        pois: vec![
            make_source_poi(11, -3.20, 55.95),
            make_source_poi(WAY_ID_PREFIX | 22, -3.10, 56.00),
            make_source_poi(RELATION_ID_PREFIX | 33, -3.31, 55.95),
            make_source_poi(44, -3.20, f64::NAN),
        ],
    }
}

#[rstest]
#[tokio::test]
async fn ingest_replays_existing_provenance_without_reingesting() {
    let request = request();
    let existing = OsmIngestionProvenanceRecord {
        geofence_id: request.geofence_id.clone(),
        source_url: request.source_url.clone(),
        input_digest: request.input_digest.clone(),
        imported_at: fixture_timestamp(),
        geofence_bounds: request.geofence_bounds,
        raw_poi_count: 9,
        filtered_poi_count: 3,
    };

    let mut source_repo = MockOsmSourceRepository::new();
    source_repo.expect_ingest_osm_pbf().times(0);

    let mut provenance_repo = MockOsmIngestionProvenanceRepository::new();
    provenance_repo
        .expect_find_by_rerun_key()
        .times(1)
        .return_once(move |_, _| Ok(Some(existing)));
    provenance_repo.expect_persist_ingestion().times(0);

    let service = make_service(source_repo, provenance_repo);
    let outcome = service
        .ingest(request)
        .await
        .expect("replay should succeed");

    assert_eq!(outcome.status, OsmIngestionStatus::Replayed);
    assert_eq!(outcome.raw_poi_count, 9);
    assert_eq!(outcome.persisted_poi_count, 3);
}

fn assert_filtered_poi_records(records: &[OsmPoiIngestionRecord]) -> bool {
    records.len() == 2
        && records
            .iter()
            .any(|record| record.element_type == "node" && record.element_id == 11)
        && records
            .iter()
            .any(|record| record.element_type == "way" && record.element_id == 22)
}

fn assert_provenance_record_matches_expected(record: &OsmIngestionProvenanceRecord) -> bool {
    record.geofence_id == "launch-a"
        && record.source_url == SOURCE_URL
        && record.input_digest == INPUT_DIGEST
        && record.geofence_bounds == GEOFENCE_BOUNDS
        && record.raw_poi_count == 4
        && record.filtered_poi_count == 2
}

#[rstest]
#[tokio::test]
async fn ingest_filters_pois_by_geofence_and_persists_provenance() {
    let request = request();

    let mut source_repo = MockOsmSourceRepository::new();
    source_repo
        .expect_ingest_osm_pbf()
        .times(1)
        .return_once(|_| Ok(fixture_source_report_for_geofence_filtering()));

    let mut provenance_repo = MockOsmIngestionProvenanceRepository::new();
    provenance_repo
        .expect_find_by_rerun_key()
        .times(1)
        .return_once(|_, _| Ok(None));
    provenance_repo
        .expect_persist_ingestion()
        .times(1)
        .withf(|provenance, poi_records| {
            assert_provenance_record_matches_expected(provenance)
                && assert_filtered_poi_records(poi_records)
        })
        .return_once(|_, _| Ok(()));

    let service = make_service(source_repo, provenance_repo);
    let outcome = service
        .ingest(request)
        .await
        .expect("ingest should succeed");

    assert_eq!(outcome.status, OsmIngestionStatus::Executed);
    assert_eq!(outcome.raw_poi_count, 4);
    assert_eq!(outcome.persisted_poi_count, 2);
    assert_eq!(outcome.geofence_bounds, GEOFENCE_BOUNDS);
}

#[rstest]
#[tokio::test]
async fn ingest_replays_on_provenance_conflict() {
    let request = request();
    let existing = OsmIngestionProvenanceRecord {
        geofence_id: request.geofence_id.clone(),
        source_url: request.source_url.clone(),
        input_digest: request.input_digest.clone(),
        imported_at: fixture_timestamp(),
        geofence_bounds: request.geofence_bounds,
        raw_poi_count: 5,
        filtered_poi_count: 1,
    };

    let mut source_repo = MockOsmSourceRepository::new();
    source_repo
        .expect_ingest_osm_pbf()
        .times(1)
        .return_once(|_| {
            Ok(OsmSourceReport {
                pois: vec![make_source_poi(7, -3.20, 55.95)],
            })
        });

    let mut provenance_repo = MockOsmIngestionProvenanceRepository::new();
    let mut sequence = Sequence::new();
    provenance_repo
        .expect_find_by_rerun_key()
        .times(1)
        .in_sequence(&mut sequence)
        .return_once(|_, _| Ok(None));
    provenance_repo
        .expect_persist_ingestion()
        .times(1)
        .in_sequence(&mut sequence)
        .return_once(|_, _| {
            Err(OsmIngestionProvenanceRepositoryError::Conflict {
                message: "duplicate rerun key".to_owned(),
            })
        });
    provenance_repo
        .expect_find_by_rerun_key()
        .times(1)
        .in_sequence(&mut sequence)
        .return_once(move |_, _| Ok(Some(existing)));

    let service = make_service(source_repo, provenance_repo);
    let outcome = service
        .ingest(request)
        .await
        .expect("conflict replay should succeed");

    assert_eq!(outcome.status, OsmIngestionStatus::Replayed);
    assert_eq!(outcome.raw_poi_count, 5);
    assert_eq!(outcome.persisted_poi_count, 1);
}

#[rstest]
#[tokio::test]
async fn ingest_returns_service_unavailable_when_conflict_lookup_is_missing() {
    let request = request();

    let mut source_repo = MockOsmSourceRepository::new();
    source_repo
        .expect_ingest_osm_pbf()
        .times(1)
        .return_once(|_| {
            Ok(OsmSourceReport {
                pois: vec![make_source_poi(7, -3.20, 55.95)],
            })
        });

    let mut provenance_repo = MockOsmIngestionProvenanceRepository::new();
    let mut sequence = Sequence::new();
    provenance_repo
        .expect_find_by_rerun_key()
        .times(1)
        .in_sequence(&mut sequence)
        .return_once(|_, _| Ok(None));
    provenance_repo
        .expect_persist_ingestion()
        .times(1)
        .in_sequence(&mut sequence)
        .return_once(|_, _| {
            Err(OsmIngestionProvenanceRepositoryError::Conflict {
                message: "duplicate rerun key".to_owned(),
            })
        });
    provenance_repo
        .expect_find_by_rerun_key()
        .times(1)
        .in_sequence(&mut sequence)
        .return_once(|_, _| Ok(None));

    let service = make_service(source_repo, provenance_repo);
    let error = service
        .ingest(request)
        .await
        .expect_err("missing conflict follow-up should fail");

    assert_eq!(error.code(), ErrorCode::ServiceUnavailable);
    assert!(
        error
            .message()
            .contains("ingestion provenance conflict occurred but rerun key was not found")
    );
}

#[rstest]
#[tokio::test]
async fn ingest_maps_atomic_persistence_failures_to_service_unavailable() {
    let request = request();

    let mut source_repo = MockOsmSourceRepository::new();
    source_repo
        .expect_ingest_osm_pbf()
        .times(1)
        .return_once(|_| {
            Ok(OsmSourceReport {
                pois: vec![make_source_poi(7, -3.20, 55.95)],
            })
        });

    let mut provenance_repo = MockOsmIngestionProvenanceRepository::new();
    provenance_repo
        .expect_find_by_rerun_key()
        .times(1)
        .return_once(|_, _| Ok(None));
    provenance_repo
        .expect_persist_ingestion()
        .times(1)
        .return_once(|_, _| {
            Err(OsmIngestionProvenanceRepositoryError::Query {
                message: "transaction failed".to_owned(),
            })
        });

    let service = make_service(source_repo, provenance_repo);
    let error = service
        .ingest(request)
        .await
        .expect_err("atomic persistence failure should fail");

    assert_eq!(error.code(), ErrorCode::ServiceUnavailable);
    assert!(
        error
            .message()
            .contains("failed to persist ingestion provenance")
    );
}

#[rstest]
#[case::read(OsmSourceRepositoryError::Read {
    message: "missing source file".to_owned(),
})]
#[case::decode(OsmSourceRepositoryError::Decode {
    message: "invalid pbf payload".to_owned(),
})]
#[tokio::test]
async fn ingest_maps_source_failures_to_service_unavailable(
    #[case] source_error: OsmSourceRepositoryError,
) {
    let request = request();

    let mut source_repo = MockOsmSourceRepository::new();
    source_repo
        .expect_ingest_osm_pbf()
        .times(1)
        .return_once(move |_| Err(source_error));

    let mut provenance_repo = MockOsmIngestionProvenanceRepository::new();
    provenance_repo
        .expect_find_by_rerun_key()
        .times(1)
        .return_once(|_, _| Ok(None));
    provenance_repo.expect_persist_ingestion().times(0);

    let service = make_service(source_repo, provenance_repo);
    let error = service
        .ingest(request)
        .await
        .expect_err("source failure should map");

    assert_eq!(error.code(), ErrorCode::ServiceUnavailable);
    assert!(error.message().contains("failed to ingest OSM source"));
}
