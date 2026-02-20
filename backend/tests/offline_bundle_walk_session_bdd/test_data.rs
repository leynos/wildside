//! Test data builders for offline bundle and walk session behavioural tests.

use backend::domain::{
    BoundingBox, OfflineBundle, OfflineBundleDraft, OfflineBundleKind, OfflineBundleStatus, UserId,
    WalkPrimaryStat, WalkPrimaryStatKind, WalkSecondaryStat, WalkSecondaryStatKind, WalkSession,
    WalkSessionDraft, ZoomRange,
};
use chrono::{Duration, Utc};
use uuid::Uuid;

pub fn build_route_bundle(owner_user_id: UserId, route_id: Uuid) -> OfflineBundle {
    let now = Utc::now();
    OfflineBundle::new(OfflineBundleDraft {
        id: Uuid::new_v4(),
        owner_user_id: Some(owner_user_id),
        device_id: "android-phone".to_owned(),
        kind: OfflineBundleKind::Route,
        route_id: Some(route_id),
        region_id: None,
        bounds: BoundingBox::new(-3.24, 55.92, -3.12, 55.99).expect("valid bounds"),
        zoom_range: ZoomRange::new(12, 16).expect("valid zoom range"),
        estimated_size_bytes: 42_000,
        created_at: now,
        updated_at: now,
        status: OfflineBundleStatus::Complete,
        progress: 1.0,
    })
    .expect("valid route bundle")
}

pub fn build_region_bundle() -> OfflineBundle {
    let now = Utc::now();
    OfflineBundle::new(OfflineBundleDraft {
        id: Uuid::new_v4(),
        owner_user_id: None,
        device_id: "tablet-offline".to_owned(),
        kind: OfflineBundleKind::Region,
        route_id: None,
        region_id: Some("edinburgh-old-town".to_owned()),
        bounds: BoundingBox::new(-3.22, 55.93, -3.16, 55.97).expect("valid bounds"),
        zoom_range: ZoomRange::new(10, 14).expect("valid zoom range"),
        estimated_size_bytes: 9_000,
        created_at: now,
        updated_at: now,
        status: OfflineBundleStatus::Queued,
        progress: 0.0,
    })
    .expect("valid region bundle")
}

pub fn build_walk_session(user_id: UserId, route_id: Uuid) -> WalkSession {
    let started_at = Utc::now();
    WalkSession::new(WalkSessionDraft {
        id: Uuid::new_v4(),
        user_id,
        route_id,
        started_at,
        ended_at: Some(started_at + Duration::minutes(47)),
        primary_stats: vec![
            WalkPrimaryStat::new(WalkPrimaryStatKind::Distance, 3650.0)
                .expect("valid distance stat"),
            WalkPrimaryStat::new(WalkPrimaryStatKind::Duration, 2820.0)
                .expect("valid duration stat"),
        ],
        secondary_stats: vec![
            WalkSecondaryStat::new(
                WalkSecondaryStatKind::Energy,
                320.0,
                Some("kcal".to_owned()),
            )
            .expect("valid energy stat"),
            WalkSecondaryStat::new(WalkSecondaryStatKind::Count, 12.0, None)
                .expect("valid count stat"),
        ],
        highlighted_poi_ids: Vec::new(),
    })
    .expect("valid walk session")
}
