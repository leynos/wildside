//! Behavioural tests for admin enrichment provenance reporting endpoint.

// Shared test doubles include helpers unused in this specific crate.
#[expect(
    clippy::type_complexity,
    reason = "Shared test doubles include helpers unused in this specific crate."
)]
#[expect(
    dead_code,
    reason = "Shared test doubles include helpers unused in this specific crate."
)]
#[path = "adapter_guardrails/doubles.rs"]
mod doubles;
// Shared helpers include functions used only by other integration suites.
#[expect(
    dead_code,
    reason = "Shared helpers include functions used only by other integration suites."
)]
#[path = "support/bdd_common.rs"]
mod bdd_common;
#[expect(
    dead_code,
    reason = "Shared harness has extra fields used by other integration suites."
)]
#[path = "adapter_guardrails/harness.rs"]
mod harness;
#[path = "support/pwa_http.rs"]
mod pwa_http;
#[path = "support/ws.rs"]
mod ws_support;

use actix_web::http::Method;
use backend::domain::ports::{
    EnrichmentProvenanceCursor, EnrichmentProvenanceRecord, EnrichmentProvenanceRepositoryError,
    ListEnrichmentProvenanceRequest, ListEnrichmentProvenanceResponse,
};
use chrono::{DateTime, Utc};
use doubles::EnrichmentProvenanceListResponse;
use harness::WorldFixture;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use serde_json::Value;
use uuid::Uuid;

const ENDPOINT: &str = "/api/v1/admin/enrichment/provenance";

#[fixture]
fn world() -> WorldFixture {
    harness::world()
}

#[path = "admin_enrichment_reporting_bdd/steps_auth.rs"]
mod steps_auth;
#[path = "admin_enrichment_reporting_bdd/steps_pagination.rs"]
mod steps_pagination;
#[path = "admin_enrichment_reporting_bdd/steps_validation.rs"]
mod steps_validation;

fn fixture_timestamp(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .expect("valid fixture timestamp")
        .with_timezone(&Utc)
}

fn fixture_record(
    source_url: &str,
    imported_at: DateTime<Utc>,
    bounding_box: [f64; 4],
) -> EnrichmentProvenanceRecord {
    EnrichmentProvenanceRecord {
        job_id: Uuid::from_u128(source_url.bytes().fold(0u128, |acc, byte| {
            acc.wrapping_mul(16_777_619).wrapping_add(u128::from(byte))
        })),
        source_url: source_url.to_owned(),
        imported_at,
        bounding_box,
    }
}

fn fixture_cursor(imported_at: DateTime<Utc>, id: Uuid) -> EnrichmentProvenanceCursor {
    EnrichmentProvenanceCursor::new(imported_at, id)
}

fn assert_record_payload(
    record: &Value,
    expected_source_url: &str,
    expected_imported_at: DateTime<Utc>,
    expected_bounding_box: [f64; 4],
) {
    assert_eq!(
        record.get("sourceUrl").and_then(Value::as_str),
        Some(expected_source_url)
    );
    assert_eq!(
        record.get("importedAt").and_then(Value::as_str),
        Some(expected_imported_at.to_rfc3339().as_str())
    );

    let bounding = record
        .get("boundingBox")
        .and_then(Value::as_object)
        .expect("boundingBox object");
    assert_eq!(
        bounding.get("minLng").and_then(Value::as_f64),
        Some(expected_bounding_box[0])
    );
    assert_eq!(
        bounding.get("minLat").and_then(Value::as_f64),
        Some(expected_bounding_box[1])
    );
    assert_eq!(
        bounding.get("maxLng").and_then(Value::as_f64),
        Some(expected_bounding_box[2])
    );
    assert_eq!(
        bounding.get("maxLat").and_then(Value::as_f64),
        Some(expected_bounding_box[3])
    );
}

fn assert_records_sorted_newest_first(records: &[Value]) {
    let imported_at = records
        .iter()
        .map(|record| {
            record
                .get("importedAt")
                .and_then(Value::as_str)
                .expect("importedAt")
        })
        .map(|value| DateTime::parse_from_rfc3339(value).expect("record importedAt RFC3339"))
        .collect::<Vec<_>>();

    assert!(
        imported_at.windows(2).all(|pair| pair[0] >= pair[1]),
        "expected records to be sorted newest-first by importedAt"
    );
}

fn perform_provenance_request(world: &WorldFixture, query: &str) {
    bdd_common::perform_get_request(world, &format!("{}?{}", ENDPOINT, query));
}

fn assert_single_provenance_call(world: &WorldFixture, expected: ListEnrichmentProvenanceRequest) {
    let ctx = world.world();
    let ctx = ctx.borrow();
    let calls = ctx.enrichment_provenance.calls();
    assert_eq!(
        calls.len(),
        1,
        "expected exactly one provenance repository call"
    );
    assert_eq!(calls[0], expected);
}

#[scenario(path = "tests/features/admin_enrichment_reporting.feature")]
fn admin_enrichment_reporting(world: WorldFixture) {
    drop(world);
}
