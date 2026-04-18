//! Local test fixtures for state-builder composition checks.
//!
//! These helpers are only used by `state_builders_composition_unit`, so they
//! live in a file-local support module rather than the shared integration-test
//! support facade.

use std::net::SocketAddr;

use actix_web::cookie::{Key, SameSite};
use backend::test_support::server::ServerConfig;
use backend::{
    domain::ports::{OfflineBundlePayload, WalkSessionPayload},
    domain::{
        BoundingBox, OfflineBundleKind, OfflineBundleStatus, UserId, WalkPrimaryStatDraft,
        WalkPrimaryStatKind, WalkSecondaryStatDraft, WalkSecondaryStatKind,
    },
};
use chrono::{DateTime, Utc};
use rstest::fixture;
use uuid::Uuid;

/// Helper to construct a fixture-mode `ServerConfig` with no database pool.
#[fixture]
pub fn fixture_config() -> ServerConfig {
    let addr: SocketAddr = "127.0.0.1:8080".parse().expect("valid addr");
    ServerConfig::new(Key::generate(), false, SameSite::Lax, addr)
}

/// Return a stable fixture timestamp for state-builder composition tests.
pub fn fixture_timestamp() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-01-02T03:04:05Z")
        .expect("RFC3339 fixture timestamp")
        .with_timezone(&Utc)
}

/// Build a representative offline bundle payload for fixture composition tests.
pub fn sample_bundle_payload(user_id: &UserId, route_id: Uuid) -> OfflineBundlePayload {
    let timestamp = fixture_timestamp();
    OfflineBundlePayload {
        id: Uuid::new_v4(),
        owner_user_id: Some(user_id.clone()),
        device_id: "fixture-device".to_owned(),
        kind: OfflineBundleKind::Route,
        route_id: Some(route_id),
        region_id: None,
        bounds: BoundingBox::new(-3.2, 55.9, -3.0, 56.0).expect("valid bounds"),
        zoom_range: backend::domain::ZoomRange::new(11, 15).expect("valid zoom range"),
        estimated_size_bytes: 1_500,
        created_at: timestamp,
        updated_at: timestamp,
        status: OfflineBundleStatus::Queued,
        progress: 0.0,
    }
}

/// Build a representative walk session payload for fixture composition tests.
pub fn sample_walk_session(user_id: &UserId, route_id: Uuid) -> WalkSessionPayload {
    let started_at = fixture_timestamp();
    WalkSessionPayload {
        id: Uuid::new_v4(),
        user_id: user_id.clone(),
        route_id,
        started_at,
        ended_at: Some(started_at),
        primary_stats: vec![WalkPrimaryStatDraft {
            kind: WalkPrimaryStatKind::Distance,
            value: 1000.0,
        }],
        secondary_stats: vec![WalkSecondaryStatDraft {
            kind: WalkSecondaryStatKind::Energy,
            value: 120.0,
            unit: Some("kcal".to_owned()),
        }],
        highlighted_poi_ids: vec![Uuid::new_v4()],
    }
}
