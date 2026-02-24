//! Unit coverage for OSM ingestion orchestration.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, TimeZone, Utc};
use mockable::DefaultClock;
use mockall::Sequence;
use rstest::{fixture, rstest};

use super::{
    RELATION_ID_PREFIX, WAY_ID_PREFIX, decode_element_id, geofence_contains, validate_request,
};
use crate::domain::ports::{
    MockOsmIngestionProvenanceRepository, MockOsmPoiRepository, MockOsmSourceRepository,
    OsmIngestionCommand, OsmIngestionProvenanceRecord, OsmIngestionProvenanceRepositoryError,
    OsmIngestionRequest, OsmIngestionStatus, OsmSourcePoi, OsmSourceReport,
    OsmSourceRepositoryError,
};
use crate::domain::{ErrorCode, OsmIngestionCommandService};

const INPUT_DIGEST: &str = "2e7d2c03a9507ae265ecf5b5356885a53393a2029f7c98f0f8f9f8f2a5f1f7c6";
const SOURCE_URL: &str = "https://example.test/launch.osm.pbf";
const GEOFENCE_BOUNDS: [f64; 4] = [-3.30, 55.90, -3.10, 56.00];

fn fixture_timestamp() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 2, 24, 10, 30, 0)
        .single()
        .expect("valid fixture timestamp")
}

fn make_source_poi(encoded_element_id: u64, longitude: f64, latitude: f64) -> OsmSourcePoi {
    let tags = BTreeMap::from([("name".to_owned(), "Fixture POI".to_owned())]);
    OsmSourcePoi {
        encoded_element_id,
        longitude,
        latitude,
        tags,
    }
}

#[fixture]
fn request() -> OsmIngestionRequest {
    OsmIngestionRequest {
        osm_pbf_path: PathBuf::from("fixtures/launch.osm.pbf"),
        source_url: SOURCE_URL.to_owned(),
        geofence_id: "launch-a".to_owned(),
        geofence_bounds: GEOFENCE_BOUNDS,
        input_digest: INPUT_DIGEST.to_owned(),
    }
}

fn make_service(
    source_repo: MockOsmSourceRepository,
    poi_repo: MockOsmPoiRepository,
    provenance_repo: MockOsmIngestionProvenanceRepository,
) -> OsmIngestionCommandService<
    MockOsmSourceRepository,
    MockOsmPoiRepository,
    MockOsmIngestionProvenanceRepository,
> {
    OsmIngestionCommandService::new(
        Arc::new(source_repo),
        Arc::new(poi_repo),
        Arc::new(provenance_repo),
        Arc::new(DefaultClock),
    )
}

#[rstest]
#[tokio::test]
async fn ingest_replays_existing_provenance_without_reingesting(request: OsmIngestionRequest) {
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

    let mut poi_repo = MockOsmPoiRepository::new();
    poi_repo.expect_upsert_pois().times(0);

    let mut provenance_repo = MockOsmIngestionProvenanceRepository::new();
    provenance_repo
        .expect_find_by_rerun_key()
        .times(1)
        .return_once(move |_, _| Ok(Some(existing)));
    provenance_repo.expect_insert().times(0);

    let service = make_service(source_repo, poi_repo, provenance_repo);
    let outcome = service
        .ingest(request)
        .await
        .expect("replay should succeed");

    assert_eq!(outcome.status, OsmIngestionStatus::Replayed);
    assert_eq!(outcome.raw_poi_count, 9);
    assert_eq!(outcome.persisted_poi_count, 3);
}

#[rstest]
#[tokio::test]
async fn ingest_filters_pois_by_geofence_and_persists_provenance(request: OsmIngestionRequest) {
    let mut source_repo = MockOsmSourceRepository::new();
    source_repo
        .expect_ingest_osm_pbf()
        .times(1)
        .return_once(|_| {
            Ok(OsmSourceReport {
                pois: vec![
                    make_source_poi(11, -3.20, 55.95),
                    make_source_poi(WAY_ID_PREFIX | 22, -3.10, 56.00),
                    make_source_poi(RELATION_ID_PREFIX | 33, -3.31, 55.95),
                    make_source_poi(44, -3.20, f64::NAN),
                ],
            })
        });

    let mut poi_repo = MockOsmPoiRepository::new();
    poi_repo
        .expect_upsert_pois()
        .times(1)
        .withf(|records| {
            records.len() == 2
                && records
                    .iter()
                    .any(|record| record.element_type == "node" && record.element_id == 11)
                && records
                    .iter()
                    .any(|record| record.element_type == "way" && record.element_id == 22)
        })
        .return_once(|_| Ok(()));

    let mut provenance_repo = MockOsmIngestionProvenanceRepository::new();
    provenance_repo
        .expect_find_by_rerun_key()
        .times(1)
        .return_once(|_, _| Ok(None));
    provenance_repo
        .expect_insert()
        .times(1)
        .withf(|record| {
            record.geofence_id == "launch-a"
                && record.source_url == SOURCE_URL
                && record.input_digest == INPUT_DIGEST
                && record.geofence_bounds == GEOFENCE_BOUNDS
                && record.raw_poi_count == 4
                && record.filtered_poi_count == 2
        })
        .return_once(|_| Ok(()));

    let service = make_service(source_repo, poi_repo, provenance_repo);
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
async fn ingest_replays_on_provenance_conflict(request: OsmIngestionRequest) {
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

    let mut poi_repo = MockOsmPoiRepository::new();
    poi_repo
        .expect_upsert_pois()
        .times(1)
        .return_once(|_| Ok(()));

    let mut provenance_repo = MockOsmIngestionProvenanceRepository::new();
    let mut sequence = Sequence::new();
    provenance_repo
        .expect_find_by_rerun_key()
        .times(1)
        .in_sequence(&mut sequence)
        .return_once(|_, _| Ok(None));
    provenance_repo
        .expect_insert()
        .times(1)
        .in_sequence(&mut sequence)
        .return_once(|_| {
            Err(OsmIngestionProvenanceRepositoryError::Conflict {
                message: "duplicate rerun key".to_owned(),
            })
        });
    provenance_repo
        .expect_find_by_rerun_key()
        .times(1)
        .in_sequence(&mut sequence)
        .return_once(move |_, _| Ok(Some(existing)));

    let service = make_service(source_repo, poi_repo, provenance_repo);
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
async fn ingest_returns_service_unavailable_when_conflict_lookup_is_missing(
    request: OsmIngestionRequest,
) {
    let mut source_repo = MockOsmSourceRepository::new();
    source_repo
        .expect_ingest_osm_pbf()
        .times(1)
        .return_once(|_| {
            Ok(OsmSourceReport {
                pois: vec![make_source_poi(7, -3.20, 55.95)],
            })
        });

    let mut poi_repo = MockOsmPoiRepository::new();
    poi_repo
        .expect_upsert_pois()
        .times(1)
        .return_once(|_| Ok(()));

    let mut provenance_repo = MockOsmIngestionProvenanceRepository::new();
    let mut sequence = Sequence::new();
    provenance_repo
        .expect_find_by_rerun_key()
        .times(1)
        .in_sequence(&mut sequence)
        .return_once(|_, _| Ok(None));
    provenance_repo
        .expect_insert()
        .times(1)
        .in_sequence(&mut sequence)
        .return_once(|_| {
            Err(OsmIngestionProvenanceRepositoryError::Conflict {
                message: "duplicate rerun key".to_owned(),
            })
        });
    provenance_repo
        .expect_find_by_rerun_key()
        .times(1)
        .in_sequence(&mut sequence)
        .return_once(|_, _| Ok(None));

    let service = make_service(source_repo, poi_repo, provenance_repo);
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
#[case::read(OsmSourceRepositoryError::Read {
    message: "missing source file".to_owned(),
})]
#[case::decode(OsmSourceRepositoryError::Decode {
    message: "invalid pbf payload".to_owned(),
})]
#[tokio::test]
async fn ingest_maps_source_failures_to_service_unavailable(
    request: OsmIngestionRequest,
    #[case] source_error: OsmSourceRepositoryError,
) {
    let mut source_repo = MockOsmSourceRepository::new();
    source_repo
        .expect_ingest_osm_pbf()
        .times(1)
        .return_once(move |_| Err(source_error));

    let mut poi_repo = MockOsmPoiRepository::new();
    poi_repo.expect_upsert_pois().times(0);

    let mut provenance_repo = MockOsmIngestionProvenanceRepository::new();
    provenance_repo
        .expect_find_by_rerun_key()
        .times(1)
        .return_once(|_, _| Ok(None));
    provenance_repo.expect_insert().times(0);

    let service = make_service(source_repo, poi_repo, provenance_repo);
    let error = service
        .ingest(request)
        .await
        .expect_err("source failure should map");

    assert_eq!(error.code(), ErrorCode::ServiceUnavailable);
    assert!(error.message().contains("failed to ingest OSM source"));
}

#[rstest]
#[case::inside(-3.20, 55.95, true)]
#[case::on_min_boundary(-3.30, 55.90, true)]
#[case::on_max_boundary(-3.10, 56.00, true)]
#[case::outside_longitude(-3.31, 55.95, false)]
#[case::outside_latitude(-3.20, 56.01, false)]
#[case::nan_coordinate(-3.20, f64::NAN, false)]
fn geofence_contains_includes_boundaries_and_rejects_non_finite(
    #[case] longitude: f64,
    #[case] latitude: f64,
    #[case] expected: bool,
) {
    let actual = geofence_contains(GEOFENCE_BOUNDS, longitude, latitude);
    assert_eq!(actual, expected);
}

#[rstest]
#[case::blank_source_url("", "launch-a", INPUT_DIGEST, "sourceUrl must not be empty")]
#[case::invalid_source_url("not-a-url", "launch-a", INPUT_DIGEST, "sourceUrl must be a valid URL")]
#[case::blank_geofence_id(SOURCE_URL, " ", INPUT_DIGEST, "geofenceId must not be empty")]
#[case::invalid_digest_length(
    SOURCE_URL,
    "launch-a",
    "deadbeef",
    "inputDigest must be a 64-character lowercase hexadecimal SHA-256 digest"
)]
#[case::invalid_digest_uppercase(
    SOURCE_URL,
    "launch-a",
    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    "inputDigest must be a 64-character lowercase hexadecimal SHA-256 digest"
)]
fn validate_request_rejects_invalid_fields(
    #[case] source_url: &str,
    #[case] geofence_id: &str,
    #[case] input_digest: &str,
    #[case] expected_message: &str,
) {
    let request = OsmIngestionRequest {
        source_url: source_url.to_owned(),
        geofence_id: geofence_id.to_owned(),
        input_digest: input_digest.to_owned(),
        ..request()
    };

    let error = validate_request(&request).expect_err("invalid request should fail");
    assert_eq!(error.code(), ErrorCode::InvalidRequest);
    assert_eq!(error.message(), expected_message);
}

#[rstest]
#[case::node(10, "node", 10)]
#[case::way(WAY_ID_PREFIX | 11, "way", 11)]
#[case::relation(RELATION_ID_PREFIX | 12, "relation", 12)]
fn decode_element_id_decodes_type_prefixes(
    #[case] encoded_id: u64,
    #[case] expected_type: &str,
    #[case] expected_id: i64,
) {
    let (element_type, element_id) = decode_element_id(encoded_id).expect("decode should work");
    assert_eq!(element_type, expected_type);
    assert_eq!(element_id, expected_id);
}

#[rstest]
fn decode_element_id_interprets_high_bit_as_relation_prefix() {
    let encoded = i64::MAX as u64 + 1;
    let (element_type, element_id) =
        decode_element_id(encoded).expect("high-bit element id should decode");

    assert_eq!(element_type, "relation");
    assert_eq!(element_id, 0);
}
