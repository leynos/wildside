//! Shared test data builders for offline bundle and walk-session endpoint BDDs.

use backend::domain::ports::{OfflineBundlePayload, WalkCompletionSummaryPayload};
use backend::domain::{
    BoundingBox, OfflineBundleKind, OfflineBundleStatus, UserId, WalkPrimaryStatDraft,
    WalkPrimaryStatKind, WalkSecondaryStatDraft, WalkSecondaryStatKind, ZoomRange,
};
use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

/// Fixture user identifier used for authenticated BDD requests.
pub const AUTH_USER_ID: &str = "11111111-1111-1111-1111-111111111111";
/// Fixture offline bundle identifier shared by upsert/list/delete scenarios.
pub const BUNDLE_ID: &str = "00000000-0000-0000-0000-000000000101";
/// Fixture route identifier referenced by bundle and walk-session payloads.
pub const ROUTE_ID: &str = "00000000-0000-0000-0000-000000000202";
/// Fixture walk-session identifier used by walk-session endpoint scenarios.
pub const SESSION_ID: &str = "00000000-0000-0000-0000-000000000501";
/// Fixture highlighted point-of-interest identifier for completion summaries.
pub const HIGHLIGHTED_POI_ID: &str = "00000000-0000-0000-0000-000000000503";
/// Fixture idempotency key reused by offline mutation scenarios.
pub const IDEMPOTENCY_KEY: &str = "550e8400-e29b-41d4-a716-446655440000";

/// Parses an RFC 3339 fixture timestamp into UTC.
///
/// # Examples
///
/// ```rust,ignore
/// let timestamp = fixture_timestamp("2026-02-01T10:00:00Z");
/// assert_eq!(timestamp.to_rfc3339(), "2026-02-01T10:00:00+00:00");
/// ```
pub fn fixture_timestamp(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .expect("fixture timestamp")
        .with_timezone(&Utc)
}

/// Builds the canonical offline bundle payload used by BDD doubles.
///
/// # Examples
///
/// ```rust,ignore
/// let bundle = build_bundle_payload();
/// assert_eq!(bundle.id.to_string(), BUNDLE_ID);
/// ```
pub fn build_bundle_payload() -> OfflineBundlePayload {
    OfflineBundlePayload {
        id: Uuid::parse_str(BUNDLE_ID).expect("bundle id"),
        owner_user_id: Some(UserId::new(AUTH_USER_ID).expect("user id")),
        device_id: "ios-iphone-15".to_owned(),
        kind: OfflineBundleKind::Route,
        route_id: Some(Uuid::parse_str(ROUTE_ID).expect("route id")),
        region_id: None,
        bounds: BoundingBox::new(-3.2, 55.9, -3.0, 56.0).expect("bounds"),
        zoom_range: ZoomRange::new(11, 15).expect("zoom range"),
        estimated_size_bytes: 4_096,
        created_at: fixture_timestamp("2026-02-01T10:00:00Z"),
        updated_at: fixture_timestamp("2026-02-01T10:00:00Z"),
        status: OfflineBundleStatus::Queued,
        progress: 0.0,
    }
}

/// Builds a completion summary payload for walk-session response assertions.
///
/// # Examples
///
/// ```rust,ignore
/// let summary = build_walk_completion_summary();
/// assert_eq!(summary.session_id.to_string(), SESSION_ID);
/// ```
pub fn build_walk_completion_summary() -> WalkCompletionSummaryPayload {
    WalkCompletionSummaryPayload {
        session_id: Uuid::parse_str(SESSION_ID).expect("session id"),
        user_id: UserId::new(AUTH_USER_ID).expect("user id"),
        route_id: Uuid::parse_str(ROUTE_ID).expect("route id"),
        started_at: fixture_timestamp("2026-02-01T11:00:00Z"),
        ended_at: fixture_timestamp("2026-02-01T11:40:00Z"),
        primary_stats: vec![WalkPrimaryStatDraft {
            kind: WalkPrimaryStatKind::Distance,
            value: 1_234.0,
        }],
        secondary_stats: vec![WalkSecondaryStatDraft {
            kind: WalkSecondaryStatKind::Energy,
            value: 120.0,
            unit: Some("kcal".to_owned()),
        }],
        highlighted_poi_ids: vec![Uuid::parse_str(HIGHLIGHTED_POI_ID).expect("poi id")],
    }
}

/// Builds the JSON body used by offline bundle upsert endpoint scenarios.
///
/// # Examples
///
/// ```rust,ignore
/// let body = offline_upsert_payload_json();
/// assert_eq!(body["id"].as_str(), Some(BUNDLE_ID));
/// ```
pub fn offline_upsert_payload_json() -> Value {
    serde_json::json!({
        "id": BUNDLE_ID,
        "deviceId": "ios-iphone-15",
        "kind": "route",
        "routeId": ROUTE_ID,
        "regionId": null,
        "bounds": {
            "minLng": -3.2,
            "minLat": 55.9,
            "maxLng": -3.0,
            "maxLat": 56.0
        },
        "zoomRange": {
            "minZoom": 11,
            "maxZoom": 15
        },
        "estimatedSizeBytes": 4096,
        "createdAt": "2026-02-01T10:00:00Z",
        "updatedAt": "2026-02-01T10:00:00Z",
        "status": "queued",
        "progress": 0.0
    })
}

/// Builds the JSON body used by walk-session creation endpoint scenarios.
///
/// # Examples
///
/// ```rust,ignore
/// let body = walk_session_payload_json();
/// assert_eq!(body["id"].as_str(), Some(SESSION_ID));
/// ```
pub fn walk_session_payload_json() -> Value {
    serde_json::json!({
        "id": SESSION_ID,
        "routeId": ROUTE_ID,
        "startedAt": "2026-02-01T11:00:00Z",
        "endedAt": "2026-02-01T11:40:00Z",
        "primaryStats": [
            {"kind": "distance", "value": 1234.0},
            {"kind": "duration", "value": 2400.0}
        ],
        "secondaryStats": [
            {"kind": "energy", "value": 120.0, "unit": "kcal"}
        ],
        "highlightedPoiIds": [HIGHLIGHTED_POI_ID]
    })
}
