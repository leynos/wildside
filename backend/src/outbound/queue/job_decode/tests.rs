//! Tests for the version-aware queue decode boundary.
//!
//! These exercise the four boundary behaviours the roadmap requires: a valid
//! `v1` payload reaching a handler, an unknown `v2` payload being rejected, the
//! rejection warning naming the received version, and malformed or missing
//! version discriminants being rejected without panicking.

use super::*;
use crate::domain::jobs::{EnrichmentJob, GenerateRouteJob};
use crate::domain::ports::JobDispatchError;
use serde_json::{Value, json};
use tracing_test::traced_test;

/// A canonical `v1` route-generation payload matching the wire snapshot.
fn valid_generate_route_v1() -> Value {
    json!({
        "v": "v1",
        "requestId": "11111111-1111-1111-1111-111111111111",
        "idempotencyKey": "33333333-3333-3333-3333-333333333333",
        "userId": "22222222-2222-2222-2222-222222222222",
        "origin": { "lat": 51.5074, "lng": -0.1278 },
        "destination": { "lat": 51.5014, "lng": -0.1419 },
        "preferences": { "mode": "walking" },
        "enqueuedAt": "2026-06-14T12:00:00Z"
    })
}

/// A canonical `v1` enrichment payload matching the wire snapshot.
fn valid_enrichment_v1() -> Value {
    json!({
        "v": "v1",
        "jobId": "44444444-4444-4444-4444-444444444444",
        "idempotencyKey": "55555555-5555-5555-5555-555555555555",
        "boundingBox": [-0.2, 51.4, 0.1, 51.6],
        "tags": ["amenity", "tourism"],
        "enqueuedAt": "2026-06-14T12:30:00Z"
    })
}

#[test]
fn decode_generate_route_v1_reaches_handler() {
    let job = decode_job::<GenerateRouteJob>(&valid_generate_route_v1())
        .expect("valid v1 route payload should decode");
    assert!(
        matches!(job, GenerateRouteJob::V1(_)),
        "decoded job should be the V1 variant"
    );
}

#[test]
fn decode_enrichment_v1_reaches_handler() {
    let job = decode_job::<EnrichmentJob>(&valid_enrichment_v1())
        .expect("valid v1 enrichment payload should decode");
    assert!(
        matches!(job, EnrichmentJob::V1(_)),
        "decoded job should be the V1 variant"
    );
}

#[test]
fn decode_unknown_version_is_rejected() {
    let mut payload = valid_generate_route_v1();
    payload["v"] = json!("v2");

    let error = decode_job::<GenerateRouteJob>(&payload)
        .expect_err("an unknown envelope version must be rejected");

    let JobDispatchError::Rejected { message } = error else {
        panic!("expected JobDispatchError::Rejected, got {error:?}");
    };
    assert!(
        message.contains("v2"),
        "rejection message should name the received version: {message}"
    );
}

#[traced_test]
#[test]
fn decode_rejection_warns_with_received_version() {
    let mut payload = valid_generate_route_v1();
    payload["v"] = json!("v2");

    let _ = decode_job::<GenerateRouteJob>(&payload);

    assert!(
        logs_contain("WARN"),
        "an unknown version should emit a loud warning"
    );
    assert!(
        logs_contain("envelope_version") && logs_contain("v2"),
        "the warning should record the received version"
    );
}

#[test]
fn decode_unreadable_version_is_rejected_without_panic() {
    let mut missing_version = valid_generate_route_v1();
    missing_version
        .as_object_mut()
        .expect("payload is a JSON object")
        .remove("v");

    let mut non_string_version = valid_generate_route_v1();
    non_string_version["v"] = json!(2);

    for (case, payload) in [
        ("missing version", missing_version),
        ("non-string version", non_string_version),
    ] {
        let error = decode_job::<GenerateRouteJob>(&payload)
            .expect_err("an unreadable envelope version must be rejected");

        assert!(
            matches!(error, JobDispatchError::Rejected { .. }),
            "{case} should be rejected, got {error:?}"
        );
    }
}

#[test]
fn decode_malformed_payload_is_rejected_without_panic() {
    // A recognized `v1` version but a structurally invalid body (missing all
    // required fields) must still be rejected, not panicked on.
    let payload = json!({ "v": "v1" });

    let error =
        decode_job::<EnrichmentJob>(&payload).expect_err("a malformed v1 body must be rejected");
    assert!(
        matches!(error, JobDispatchError::Rejected { .. }),
        "malformed payload should be rejected, got {error:?}"
    );
}
