//! Boundary tests for V1 enrichment tag limits.

use pretty_assertions::assert_eq;
use rstest::rstest;
use uuid::Uuid;

use super::super::{
    ENRICHMENT_JOB_V1_MAX_TAG_LENGTH, ENRICHMENT_JOB_V1_MAX_TAGS, EnrichmentJob,
    EnrichmentJobBuildError, EnrichmentJobParams,
};
use super::{
    enrichment_job_json, fixture_bounding_box, fixture_enqueued_at, fixture_idempotency_key,
};

#[rstest]
fn constructor_accepts_exact_tag_limits() {
    let job = EnrichmentJob::v1(EnrichmentJobParams {
        job_id: fixture_job_id(),
        idempotency_key: Some(fixture_idempotency_key()),
        bounding_box: fixture_bounding_box(),
        tags: tags_at_v1_limits(),
        enqueued_at: fixture_enqueued_at(),
    })
    .expect("exact V1 tag limits should be accepted");

    assert_eq!(job.tags().len(), ENRICHMENT_JOB_V1_MAX_TAGS);
    assert!(job.tags().contains(&max_length_utf8_tag()));
}

#[rstest]
fn constructor_rejects_multibyte_tag_over_byte_limit() {
    let error = EnrichmentJob::v1(EnrichmentJobParams {
        job_id: fixture_job_id(),
        idempotency_key: Some(fixture_idempotency_key()),
        bounding_box: fixture_bounding_box(),
        tags: vec![overlong_utf8_tag()],
        enqueued_at: fixture_enqueued_at(),
    })
    .expect_err("a 65-byte tag should be rejected");

    assert_eq!(
        error,
        EnrichmentJobBuildError::TagTooLong {
            limit: ENRICHMENT_JOB_V1_MAX_TAG_LENGTH,
            observed: ENRICHMENT_JOB_V1_MAX_TAG_LENGTH + 1,
        }
    );
}

#[rstest]
fn serde_accepts_exact_tag_limits() {
    let value = enrichment_job_json(
        fixture_job_id(),
        fixture_enqueued_at(),
        serde_json::json!(tags_at_v1_limits()),
    );

    let job: EnrichmentJob =
        serde_json::from_value(value).expect("exact persisted V1 tag limits should be accepted");

    assert_eq!(job.tags().len(), ENRICHMENT_JOB_V1_MAX_TAGS);
    assert!(job.tags().contains(&max_length_utf8_tag()));
}

#[rstest]
fn serde_rejects_multibyte_tag_over_byte_limit() {
    let value = enrichment_job_json(
        fixture_job_id(),
        fixture_enqueued_at(),
        serde_json::json!([overlong_utf8_tag()]),
    );

    let error = serde_json::from_value::<EnrichmentJob>(value)
        .expect_err("a persisted 65-byte tag should be rejected");

    assert!(
        error
            .to_string()
            .contains("enrichment job tag is too long: 65 > 64"),
        "unexpected rejection: {error}"
    );
}

fn fixture_job_id() -> Uuid {
    Uuid::from_bytes([0x44; 16])
}

fn max_length_utf8_tag() -> String {
    "é".repeat(ENRICHMENT_JOB_V1_MAX_TAG_LENGTH / 2)
}

fn overlong_utf8_tag() -> String {
    format!("{}x", max_length_utf8_tag())
}

fn tags_at_v1_limits() -> Vec<String> {
    let mut tags = (0..ENRICHMENT_JOB_V1_MAX_TAGS - 1)
        .map(|index| format!("tag-{index}"))
        .collect::<Vec<_>>();
    tags.push(max_length_utf8_tag());
    tags
}
