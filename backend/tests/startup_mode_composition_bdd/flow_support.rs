//! Shared types and assertion helpers for comprehensive startup-mode
//! composition BDD.
//!
//! This module provides the `World` state struct, `Snapshot` type, and
//! assertion helpers used by both the scenario steps and the flow execution
//! functions in the sibling `flows` module.

use std::sync::Arc;

use actix_web::cookie::Cookie;
use serde_json::Value;
use uuid::Uuid;

use super::db_support::DbContext;
use super::support::profile_interests::FIXTURE_AUTH_ID;

/// Snapshot of an HTTP response for assertion purposes.
#[derive(Debug)]
pub(crate) struct Snapshot {
    pub(crate) status: u16,
    pub(crate) body: Option<Value>,
    pub(crate) trace_id: Option<String>,
    pub(crate) session_cookie: Option<Cookie<'static>>,
}

/// BDD world state tracking all endpoint responses and startup mode.
pub(crate) struct World {
    pub(crate) runtime: Arc<tokio::runtime::Runtime>,
    pub(crate) db: Option<DbContext>,
    pub(crate) seeded_route_id: Option<Uuid>,
    pub(crate) login: Option<Snapshot>,
    pub(crate) profile: Option<Snapshot>,
    pub(crate) users_list: Option<Snapshot>,
    pub(crate) interests: Option<Snapshot>,
    pub(crate) preferences: Option<Snapshot>,
    pub(crate) catalogue_explore: Option<Snapshot>,
    pub(crate) catalogue_descriptors: Option<Snapshot>,
    pub(crate) offline_bundles: Option<Snapshot>,
    pub(crate) walk_sessions: Option<Snapshot>,
    pub(crate) enrichment_provenance: Option<Snapshot>,
    pub(crate) route_annotations: Option<Snapshot>,
    pub(crate) route_submission: Option<Snapshot>,
    /// Persisted validation snapshot from the first mode run, used to compare
    /// error envelope equality across startup modes.
    pub(crate) validation_baseline: Option<ValidationBaseline>,
    pub(crate) skip_reason: Option<String>,
}

/// Structural representation of a validation error envelope, used to assert
/// that fixture and DB modes produce identical error shapes.
#[derive(Debug, PartialEq)]
pub(crate) struct ValidationBaseline {
    pub(crate) status: u16,
    pub(crate) code: String,
    pub(crate) has_details: bool,
}

/// Check if the scenario should be skipped due to cluster setup failure.
pub(crate) fn is_skipped(world: &World) -> bool {
    if let Some(reason) = world.skip_reason.as_deref() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped ({reason})");
        true
    } else {
        false
    }
}

/// Assert that a snapshot represents a 500 Internal Server Error with stable
/// error envelope.
pub(crate) fn assert_internal(snapshot: &Snapshot) {
    assert_eq!(snapshot.status, 500);
    let body = snapshot.body.as_ref().expect("error body");
    assert_eq!(
        body.get("message").and_then(Value::as_str),
        Some("Internal server error")
    );
    assert_eq!(
        body.get("code").and_then(Value::as_str),
        Some("internal_error")
    );
    let trace_header = snapshot.trace_id.as_deref().expect("trace-id header");
    let trace_body = body
        .get("traceId")
        .and_then(Value::as_str)
        .expect("traceId body");
    assert_eq!(trace_header, trace_body);
}

/// Extract a [`ValidationBaseline`] from a preferences snapshot for
/// cross-mode comparison.
pub(crate) fn extract_validation_baseline(snapshot: &Snapshot) -> ValidationBaseline {
    let code = snapshot
        .body
        .as_ref()
        .and_then(|b| b.get("code"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let has_details = snapshot
        .body
        .as_ref()
        .and_then(|b| b.get("details"))
        .is_some();
    ValidationBaseline {
        status: snapshot.status,
        code,
        has_details,
    }
}

/// Assert that a profile response matches the expected display name.
pub(crate) fn assert_profile_response(snapshot: &Snapshot, expected_display_name: &str) {
    assert_eq!(snapshot.status, 200);
    let body = snapshot.body.as_ref().expect("profile body");
    assert_eq!(
        body.get("id").and_then(Value::as_str),
        Some(FIXTURE_AUTH_ID)
    );
    assert_eq!(
        body.get("displayName").and_then(Value::as_str),
        Some(expected_display_name)
    );
}
