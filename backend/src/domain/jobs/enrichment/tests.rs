//! Tests for enrichment job payloads.

use chrono::{DateTime, Utc};
use insta::assert_json_snapshot;
use pretty_assertions::assert_eq;
use proptest::prelude::*;
use rstest::{fixture, rstest};
use uuid::Uuid;

use super::{
    ENRICHMENT_JOB_V1_MAX_TAG_LENGTH, ENRICHMENT_JOB_V1_MAX_TAGS, EnrichmentJob,
    EnrichmentJobBuildError, EnrichmentJobParams,
};
use crate::domain::IdempotencyKey;
use crate::domain::jobs::{BoundingBox, BoundingBoxError};

#[fixture]
fn job_id() -> Uuid {
    Uuid::from_bytes([0x44; 16])
}

#[fixture]
fn enqueued_at() -> DateTime<Utc> {
    fixture_enqueued_at()
}

fn fixture_enqueued_at() -> DateTime<Utc> {
    match DateTime::parse_from_rfc3339("2026-06-14T12:30:00Z") {
        Ok(timestamp) => timestamp.with_timezone(&Utc),
        Err(error) => panic!("static enqueue timestamp must be valid: {error}"),
    }
}

fn fixture_idempotency_key() -> IdempotencyKey {
    IdempotencyKey::from_uuid(Uuid::from_bytes([0x55; 16]))
}

fn fixture_bounding_box() -> BoundingBox {
    match BoundingBox::new(-0.20, 51.40, 0.10, 51.60) {
        Ok(bounding_box) => bounding_box,
        Err(error) => panic!("static bounding box must be valid: {error}"),
    }
}

#[rstest]
fn bounding_box_accepts_valid_coordinates() {
    let bounding_box = BoundingBox::new(-180.0, -90.0, 180.0, 90.0)
        .expect("world-spanning bounding box should be valid");

    assert_eq!(bounding_box.coords(), [-180.0, -90.0, 180.0, 90.0]);
}

#[derive(Clone)]
struct InvalidBoundingBoxCase {
    min_lng: f64,
    min_lat: f64,
    max_lng: f64,
    max_lat: f64,
    expected: BoundingBoxError,
}

#[rstest]
#[case(InvalidBoundingBoxCase {
    min_lng: f64::NAN,
    min_lat: 0.0,
    max_lng: 1.0,
    max_lat: 1.0,
    expected: BoundingBoxError::NonFinite,
})]
#[case(InvalidBoundingBoxCase {
    min_lng: -181.0,
    min_lat: 0.0,
    max_lng: 1.0,
    max_lat: 1.0,
    expected: BoundingBoxError::LongitudeOutOfRange,
})]
#[case(InvalidBoundingBoxCase {
    min_lng: 0.0,
    min_lat: -91.0,
    max_lng: 1.0,
    max_lat: 1.0,
    expected: BoundingBoxError::LatitudeOutOfRange,
})]
#[case(InvalidBoundingBoxCase {
    min_lng: 0.0,
    min_lat: 0.0,
    max_lng: 0.0,
    max_lat: 1.0,
    expected: BoundingBoxError::AntimeridianWrap,
})]
#[case(InvalidBoundingBoxCase {
    min_lng: 0.0,
    min_lat: 1.0,
    max_lng: 1.0,
    max_lat: 1.0,
    expected: BoundingBoxError::InvertedOrdering,
})]
fn bounding_box_rejects_invalid_coordinates(#[case] case: InvalidBoundingBoxCase) {
    let error = BoundingBox::new(case.min_lng, case.min_lat, case.max_lng, case.max_lat)
        .expect_err("invalid bounding box should be rejected");

    assert_eq!(error, case.expected);
}

#[rstest]
fn constructor_sorts_and_deduplicates_tags(job_id: Uuid, enqueued_at: DateTime<Utc>) {
    let job = EnrichmentJob::v1(EnrichmentJobParams {
        job_id,
        idempotency_key: Some(fixture_idempotency_key()),
        bounding_box: fixture_bounding_box(),
        tags: vec![
            "tourism".to_owned(),
            "amenity".to_owned(),
            "tourism".to_owned(),
        ],
        enqueued_at,
    })
    .expect("valid enrichment job should build");

    assert_eq!(job.tags(), &["amenity".to_owned(), "tourism".to_owned()]);
}

#[rstest]
fn constructor_rejects_empty_tags(job_id: Uuid, enqueued_at: DateTime<Utc>) {
    let error = EnrichmentJob::v1(EnrichmentJobParams {
        job_id,
        idempotency_key: Some(fixture_idempotency_key()),
        bounding_box: fixture_bounding_box(),
        tags: Vec::new(),
        enqueued_at,
    })
    .expect_err("empty tags should be rejected");

    assert_eq!(error, EnrichmentJobBuildError::EmptyTags);
}

#[derive(Clone)]
struct TagRejectionCase {
    tags: Vec<String>,
    expected: EnrichmentJobBuildError,
}

#[rstest]
#[case(TagRejectionCase {
    tags: (0..=ENRICHMENT_JOB_V1_MAX_TAGS)
        .map(|index| format!("tag-{index}"))
        .collect(),
    expected: EnrichmentJobBuildError::TooManyTags {
        limit: ENRICHMENT_JOB_V1_MAX_TAGS,
        observed: ENRICHMENT_JOB_V1_MAX_TAGS + 1,
    },
})]
#[case(TagRejectionCase {
    tags: vec!["x".repeat(ENRICHMENT_JOB_V1_MAX_TAG_LENGTH + 1)],
    expected: EnrichmentJobBuildError::TagTooLong {
        limit: ENRICHMENT_JOB_V1_MAX_TAG_LENGTH,
        observed: ENRICHMENT_JOB_V1_MAX_TAG_LENGTH + 1,
    },
})]
fn constructor_rejects_invalid_tags(
    job_id: Uuid,
    enqueued_at: DateTime<Utc>,
    #[case] case: TagRejectionCase,
) {
    let error = EnrichmentJob::v1(EnrichmentJobParams {
        job_id,
        idempotency_key: Some(fixture_idempotency_key()),
        bounding_box: fixture_bounding_box(),
        tags: case.tags,
        enqueued_at,
    })
    .expect_err("invalid tags should be rejected");

    assert_eq!(error, case.expected);
}

#[rstest]
fn serde_round_trip_is_identity(job_id: Uuid, enqueued_at: DateTime<Utc>) {
    let job = fixture_job(job_id, enqueued_at);

    let value = serde_json::to_value(&job).expect("job should serialize");
    let decoded: EnrichmentJob = serde_json::from_value(value).expect("job should deserialize");

    assert_eq!(decoded, job);
}

#[rstest]
fn unknown_fields_are_rejected(job_id: Uuid, enqueued_at: DateTime<Utc>) {
    let job = fixture_job(job_id, enqueued_at);
    let mut value = serde_json::to_value(job).expect("job should serialize");
    value
        .as_object_mut()
        .expect("job envelope should be an object")
        .insert("unexpected".to_owned(), serde_json::json!(true));

    let error =
        serde_json::from_value::<EnrichmentJob>(value).expect_err("unknown fields are rejected");

    assert!(error.to_string().contains("unknown field"));
}

#[rstest]
fn serde_canonicalizes_duplicate_tags(job_id: Uuid, enqueued_at: DateTime<Utc>) {
    let value = enrichment_job_json(
        job_id,
        enqueued_at,
        serde_json::json!(["tourism", "amenity", "tourism"]),
    );

    let decoded: EnrichmentJob = serde_json::from_value(value).expect("job should deserialize");

    assert_eq!(
        decoded.tags(),
        &["amenity".to_owned(), "tourism".to_owned()]
    );
}

#[derive(Clone)]
struct InvalidSerdeTagsCase {
    tags: serde_json::Value,
    expected_message: &'static str,
}

#[rstest]
#[case(InvalidSerdeTagsCase {
    tags: serde_json::json!([]),
    expected_message: "enrichment job requires at least one tag",
})]
#[case(InvalidSerdeTagsCase {
    tags: serde_json::json!(
        (0..=ENRICHMENT_JOB_V1_MAX_TAGS)
            .map(|index| format!("tag-{index}"))
            .collect::<Vec<_>>()
    ),
    expected_message: "enrichment job has too many tags",
})]
#[case(InvalidSerdeTagsCase {
    tags: serde_json::json!(["x".repeat(ENRICHMENT_JOB_V1_MAX_TAG_LENGTH + 1)]),
    expected_message: "enrichment job tag is too long",
})]
fn serde_rejects_invalid_tags(
    job_id: Uuid,
    enqueued_at: DateTime<Utc>,
    #[case] case: InvalidSerdeTagsCase,
) {
    let value = enrichment_job_json(job_id, enqueued_at, case.tags);

    let error = serde_json::from_value::<EnrichmentJob>(value)
        .expect_err("invalid persisted tags should be rejected");

    assert!(
        error.to_string().contains(case.expected_message),
        "expected error to contain {:?}, got {error}",
        case.expected_message,
    );
}

#[rstest]
fn converts_to_overpass_request(job_id: Uuid, enqueued_at: DateTime<Utc>) {
    let job = fixture_job(job_id, enqueued_at);

    let request = job.to_overpass_request();

    assert_eq!(request.job_id, job_id);
    assert_eq!(request.bounding_box, fixture_bounding_box().coords());
    assert_eq!(
        request.tags,
        vec!["amenity".to_owned(), "tourism".to_owned()]
    );
}

#[rstest]
fn snapshot_locks_v1_json_shape(job_id: Uuid, enqueued_at: DateTime<Utc>) {
    let job = fixture_job(job_id, enqueued_at);
    let value = serde_json::to_value(job).expect("job should serialize");

    assert_json_snapshot!("enrichment_job_v1", value);
}

fn fixture_job(job_id: Uuid, enqueued_at: DateTime<Utc>) -> EnrichmentJob {
    match EnrichmentJob::v1(EnrichmentJobParams {
        job_id,
        idempotency_key: Some(fixture_idempotency_key()),
        bounding_box: fixture_bounding_box(),
        tags: vec!["tourism".to_owned(), "amenity".to_owned()],
        enqueued_at,
    }) {
        Ok(job) => job,
        Err(error) => panic!("static enrichment job should be valid: {error}"),
    }
}

fn enrichment_job_json(
    job_id: Uuid,
    enqueued_at: DateTime<Utc>,
    tags: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "v": "v1",
        "jobId": job_id,
        "idempotencyKey": fixture_idempotency_key(),
        "boundingBox": fixture_bounding_box(),
        "tags": tags,
        "enqueuedAt": enqueued_at,
    })
}

fn valid_bounding_box_strategy() -> impl Strategy<Value = BoundingBox> {
    (
        -179.0_f64..179.0,
        -89.0_f64..89.0,
        0.001_f64..1.0,
        0.001_f64..1.0,
    )
        .prop_map(|(min_lng, min_lat, lng_span, lat_span)| {
            let max_lng = (min_lng + lng_span).min(180.0);
            let max_lat = (min_lat + lat_span).min(90.0);
            match BoundingBox::new(min_lng, min_lat, max_lng, max_lat) {
                Ok(bounding_box) => bounding_box,
                Err(error) => panic!("strategy should generate valid bounding boxes: {error}"),
            }
        })
}

fn valid_tags_strategy() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec("[a-z][a-z0-9_]{0,15}", 1..=8)
}

fn enrichment_job_strategy() -> impl Strategy<Value = EnrichmentJob> {
    (valid_bounding_box_strategy(), valid_tags_strategy()).prop_map(|(bounding_box, tags)| {
        match EnrichmentJob::v1(EnrichmentJobParams {
            job_id: Uuid::from_bytes([0x44; 16]),
            idempotency_key: Some(fixture_idempotency_key()),
            bounding_box,
            tags,
            enqueued_at: fixture_enqueued_at(),
        }) {
            Ok(job) => job,
            Err(error) => panic!("strategy should generate valid enrichment jobs: {error}"),
        }
    })
}

proptest! {
    #[test]
    fn generated_jobs_round_trip_through_json(job in enrichment_job_strategy()) {
        let value = serde_json::to_value(&job).expect("generated job should serialize");
        let decoded: EnrichmentJob =
            serde_json::from_value(value).expect("generated job should deserialize");

        prop_assert_eq!(decoded, job);
    }

    #[test]
    fn generated_jobs_preserve_overpass_request(job in enrichment_job_strategy()) {
        let request = job.to_overpass_request();

        prop_assert_eq!(request.bounding_box, job.bounding_box().coords());
        prop_assert_eq!(request.tags, job.tags().to_vec());
    }

    #[test]
    fn inverted_longitude_ordering_is_rejected(
        min_lng in -179.0_f64..179.0,
        min_lat in -89.0_f64..89.0,
        lat_span in 0.001_f64..1.0,
    ) {
        let error = BoundingBox::new(min_lng, min_lat, min_lng, min_lat + lat_span)
            .expect_err("equal longitudes should be rejected");

        prop_assert_eq!(error, BoundingBoxError::AntimeridianWrap);
    }
}
