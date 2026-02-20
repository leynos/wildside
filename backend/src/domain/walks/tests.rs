//! Regression coverage for walk session domain types.

use chrono::{Duration, TimeZone, Utc};
use rstest::rstest;
use uuid::Uuid;

use super::{
    WalkPrimaryStat, WalkPrimaryStatKind, WalkSecondaryStat, WalkSecondaryStatKind, WalkSession,
    WalkSessionDraft, WalkValidationError,
};
use crate::domain::UserId;

fn build_walk_session_draft() -> WalkSessionDraft {
    let started_at = Utc
        .with_ymd_and_hms(2026, 2, 20, 9, 0, 0)
        .single()
        .expect("valid timestamp");

    WalkSessionDraft {
        id: Uuid::new_v4(),
        user_id: UserId::random(),
        route_id: Uuid::new_v4(),
        started_at,
        ended_at: Some(started_at + Duration::minutes(45)),
        primary_stats: vec![
            WalkPrimaryStat::new(WalkPrimaryStatKind::Distance, 3600.0)
                .expect("valid distance stat"),
            WalkPrimaryStat::new(WalkPrimaryStatKind::Duration, 2700.0)
                .expect("valid duration stat"),
        ],
        secondary_stats: vec![
            WalkSecondaryStat::new(
                WalkSecondaryStatKind::Energy,
                220.0,
                Some("kcal".to_owned()),
            )
            .expect("valid energy stat"),
            WalkSecondaryStat::new(WalkSecondaryStatKind::Count, 18.0, None)
                .expect("valid count stat"),
        ],
        highlighted_poi_ids: vec![Uuid::new_v4(), Uuid::new_v4()],
    }
}

#[rstest]
fn walk_session_constructs_from_valid_draft() {
    let draft = build_walk_session_draft();
    let session = WalkSession::new(draft.clone()).expect("valid walk session");

    assert_eq!(session.route_id(), draft.route_id);
    assert_eq!(session.primary_stats().len(), 2);
    assert_eq!(session.secondary_stats().len(), 2);
    assert_eq!(session.highlighted_poi_ids().len(), 2);
}

#[rstest]
fn primary_stat_rejects_negative_value() {
    let result = WalkPrimaryStat::new(WalkPrimaryStatKind::Distance, -1.0);
    assert!(matches!(
        result,
        Err(WalkValidationError::NegativePrimaryStatValue {
            kind: WalkPrimaryStatKind::Distance,
            ..
        })
    ));
}

#[rstest]
fn secondary_stat_rejects_negative_value() {
    let result =
        WalkSecondaryStat::new(WalkSecondaryStatKind::Energy, -5.0, Some("kcal".to_owned()));
    assert!(matches!(
        result,
        Err(WalkValidationError::NegativeSecondaryStatValue {
            kind: WalkSecondaryStatKind::Energy,
            ..
        })
    ));
}

#[rstest]
fn secondary_stat_rejects_blank_unit() {
    let result =
        WalkSecondaryStat::new(WalkSecondaryStatKind::Energy, 10.0, Some("   ".to_owned()));
    assert!(matches!(
        result,
        Err(WalkValidationError::EmptySecondaryStatUnit)
    ));
}

#[rstest]
fn session_rejects_ended_at_before_started_at() {
    let mut draft = build_walk_session_draft();
    draft.ended_at = Some(draft.started_at - Duration::seconds(1));

    let result = WalkSession::new(draft);
    assert!(matches!(
        result,
        Err(WalkValidationError::EndedBeforeStarted)
    ));
}

#[rstest]
fn session_rejects_duplicate_primary_stat_kinds() {
    let mut draft = build_walk_session_draft();
    draft.primary_stats = vec![
        WalkPrimaryStat::new(WalkPrimaryStatKind::Distance, 1200.0).expect("valid primary stat"),
        WalkPrimaryStat::new(WalkPrimaryStatKind::Distance, 900.0).expect("valid primary stat"),
    ];

    let result = WalkSession::new(draft);
    assert!(matches!(
        result,
        Err(WalkValidationError::DuplicatePrimaryStatKind {
            kind: WalkPrimaryStatKind::Distance
        })
    ));
}

#[rstest]
fn session_rejects_duplicate_secondary_stat_kinds() {
    let mut draft = build_walk_session_draft();
    draft.secondary_stats = vec![
        WalkSecondaryStat::new(WalkSecondaryStatKind::Count, 10.0, None)
            .expect("valid secondary stat"),
        WalkSecondaryStat::new(WalkSecondaryStatKind::Count, 14.0, None)
            .expect("valid secondary stat"),
    ];

    let result = WalkSession::new(draft);
    assert!(matches!(
        result,
        Err(WalkValidationError::DuplicateSecondaryStatKind {
            kind: WalkSecondaryStatKind::Count
        })
    ));
}

#[rstest]
fn session_rejects_duplicate_highlighted_poi_ids() {
    let mut draft = build_walk_session_draft();
    let duplicate = Uuid::new_v4();
    draft.highlighted_poi_ids = vec![duplicate, duplicate];

    let result = WalkSession::new(draft);
    assert!(matches!(
        result,
        Err(WalkValidationError::DuplicateHighlightedPoiId { .. })
    ));
}

#[rstest]
fn completion_summary_requires_completed_session() {
    let mut draft = build_walk_session_draft();
    draft.ended_at = None;
    let session = WalkSession::new(draft).expect("session without end is valid");

    let result = session.completion_summary();
    assert!(matches!(
        result,
        Err(WalkValidationError::SessionNotCompleted)
    ));
}

#[rstest]
fn completion_summary_contains_session_payload() {
    let session = WalkSession::new(build_walk_session_draft()).expect("valid session");
    let summary = session
        .completion_summary()
        .expect("completed session should produce summary");

    assert_eq!(summary.session_id(), session.id());
    assert_eq!(summary.route_id(), session.route_id());
    assert_eq!(summary.primary_stats().len(), session.primary_stats().len());
    assert_eq!(
        summary.highlighted_poi_ids().len(),
        session.highlighted_poi_ids().len()
    );
}
