//! Tests for the version-aware queue decode boundary.
//!
//! These exercise the boundary behaviours the roadmap requires: a valid `v1`
//! payload reaching a handler, an unknown `v2` payload being rejected, and
//! malformed or missing version discriminants being rejected without
//! panicking.

use super::*;
use crate::domain::jobs::{EnrichmentJob, GenerateRouteJob};
use crate::domain::ports::JobDispatchError;
use rstest::rstest;
use serde_json::{Value, json};

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

    assert_eq!(
        rejection_message(error),
        "unsupported job envelope version: v2"
    );
}

#[rstest]
#[case::missing_version("missing version", json!({}))]
#[case::non_string_version("non-string version", json!({ "v": 2 }))]
fn decode_unreadable_version_is_rejected_without_panic(#[case] case: &str, #[case] payload: Value) {
    let error = decode_job::<GenerateRouteJob>(&payload)
        .expect_err("an unreadable envelope version must be rejected");

    assert_eq!(
        rejection_message(error),
        "malformed job payload",
        "{case} should use the fixed malformed-payload diagnostic"
    );
}

#[test]
fn decode_malformed_payload_is_rejected_without_panic() {
    // A recognized `v1` version but a structurally invalid body (missing all
    // required fields) must still be rejected, not panicked on.
    let payload = json!({ "v": "v1" });

    let error =
        decode_job::<EnrichmentJob>(&payload).expect_err("a malformed v1 body must be rejected");
    assert_eq!(rejection_message(error), "malformed job payload");
}

#[test]
fn decode_caps_untrusted_version_diagnostics() {
    const EXPECTED_DIAGNOSTIC_LENGTH: usize = 64;
    let mut payload = valid_generate_route_v1();
    payload["v"] = json!("v".repeat(EXPECTED_DIAGNOSTIC_LENGTH * 100));

    let error = decode_job::<GenerateRouteJob>(&payload)
        .expect_err("an oversized unknown version must be rejected");

    let message = rejection_message(error);
    assert_eq!(
        message,
        format!(
            "unsupported job envelope version: {}",
            "v".repeat(EXPECTED_DIAGNOSTIC_LENGTH)
        )
    );

    let expected_utf8_prefix = "v".repeat(EXPECTED_DIAGNOSTIC_LENGTH - 1);
    payload["v"] = json!(format!("{expected_utf8_prefix}é"));

    let error = decode_job::<GenerateRouteJob>(&payload)
        .expect_err("an unknown version crossing the byte limit must be rejected");
    let message = rejection_message(error);
    assert_eq!(
        message,
        format!("unsupported job envelope version: {expected_utf8_prefix}")
    );
}

#[rstest]
#[case::newline("v2\n", r"v2\n")]
#[case::carriage_return("v2\r", r"v2\r")]
#[case::terminal_escape("v2\u{1b}", r"v2\x1b")]
#[case::line_separator("v2\u{2028}", r"v2\u{2028}")]
#[case::paragraph_separator("v2\u{2029}", r"v2\u{2029}")]
fn decode_escapes_control_characters_in_version_diagnostics(
    #[case] version: &str,
    #[case] expected_diagnostic: &str,
) {
    let mut payload = valid_generate_route_v1();
    payload["v"] = json!(version);

    let error = decode_job::<GenerateRouteJob>(&payload)
        .expect_err("an unsupported control-bearing version must be rejected");
    let message = rejection_message(error);

    assert_eq!(
        message,
        format!("unsupported job envelope version: {expected_diagnostic}")
    );
    assert!(
        !message.chars().any(char::is_control),
        "rejection diagnostics must not contain raw control characters: {message:?}"
    );
}

#[test]
fn decode_caps_escaped_control_heavy_version_diagnostics() {
    let mut payload = valid_generate_route_v1();
    payload["v"] = json!("v2\n".repeat(100));

    let error = decode_job::<GenerateRouteJob>(&payload)
        .expect_err("an oversized control-bearing version must be rejected");
    let message = rejection_message(error);
    let diagnostic = message
        .strip_prefix("unsupported job envelope version: ")
        .expect("rejection should use the unsupported-version prefix");

    assert!(diagnostic.len() <= MAX_VERSION_DIAGNOSTIC_BYTES);
    assert!(!diagnostic.chars().any(char::is_control));
}

fn rejection_message(error: JobDispatchError) -> String {
    let JobDispatchError::Rejected { message } = error else {
        panic!("expected JobDispatchError::Rejected, got {error:?}");
    };
    message
}
