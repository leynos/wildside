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
#[case(WalkPrimaryStatKind::Distance)]
#[case(WalkPrimaryStatKind::Duration)]
fn primary_stat_rejects_negative_value(#[case] kind: WalkPrimaryStatKind) {
    let result = WalkPrimaryStat::new(kind, -1.0);
    assert!(matches!(
        result,
        Err(WalkValidationError::NegativePrimaryStatValue {
            kind: actual_kind,
            ..
        }) if actual_kind == kind
    ));
}

#[rstest]
#[case(WalkPrimaryStatKind::Distance, f64::NAN)]
#[case(WalkPrimaryStatKind::Distance, f64::INFINITY)]
#[case(WalkPrimaryStatKind::Duration, f64::NEG_INFINITY)]
fn primary_stat_rejects_non_finite_value(#[case] kind: WalkPrimaryStatKind, #[case] value: f64) {
    let result = WalkPrimaryStat::new(kind, value);
    assert!(matches!(
        result,
        Err(WalkValidationError::NegativePrimaryStatValue {
            kind: actual_kind,
            ..
        }) if actual_kind == kind
    ));
}

#[rstest]
#[case(WalkSecondaryStatKind::Energy, Some("kcal"))]
#[case(WalkSecondaryStatKind::Count, None)]
fn secondary_stat_rejects_negative_value(
    #[case] kind: WalkSecondaryStatKind,
    #[case] unit: Option<&str>,
) {
    let result = WalkSecondaryStat::new(kind, -5.0, unit.map(str::to_owned));
    assert!(matches!(
        result,
        Err(WalkValidationError::NegativeSecondaryStatValue {
            kind: actual_kind,
            ..
        }) if actual_kind == kind
    ));
}

#[rstest]
#[case(WalkSecondaryStatKind::Energy, f64::NAN, Some("kcal"))]
#[case(WalkSecondaryStatKind::Energy, f64::INFINITY, Some("kcal"))]
#[case(WalkSecondaryStatKind::Count, f64::NEG_INFINITY, None)]
fn secondary_stat_rejects_non_finite_value(
    #[case] kind: WalkSecondaryStatKind,
    #[case] value: f64,
    #[case] unit: Option<&str>,
) {
    let result = WalkSecondaryStat::new(kind, value, unit.map(str::to_owned));
    assert!(matches!(
        result,
        Err(WalkValidationError::NegativeSecondaryStatValue {
            kind: actual_kind,
            ..
        }) if actual_kind == kind
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
#[case(WalkPrimaryStatKind::Distance)]
#[case(WalkPrimaryStatKind::Duration)]
fn session_rejects_duplicate_primary_stat_kinds(#[case] kind: WalkPrimaryStatKind) {
    let mut draft = build_walk_session_draft();
    draft.primary_stats = vec![
        WalkPrimaryStat::new(kind, 1200.0).expect("valid primary stat"),
        WalkPrimaryStat::new(kind, 900.0).expect("valid primary stat"),
    ];

    let result = WalkSession::new(draft);
    assert!(matches!(
        result,
        Err(WalkValidationError::DuplicatePrimaryStatKind {
            kind: actual_kind
        }) if actual_kind == kind
    ));
}

#[rstest]
#[case(WalkSecondaryStatKind::Energy, Some("kcal"), Some("kJ"))]
#[case(WalkSecondaryStatKind::Count, None, None)]
fn session_rejects_duplicate_secondary_stat_kinds(
    #[case] kind: WalkSecondaryStatKind,
    #[case] first_unit: Option<&str>,
    #[case] second_unit: Option<&str>,
) {
    let mut draft = build_walk_session_draft();
    draft.secondary_stats = vec![
        WalkSecondaryStat::new(kind, 10.0, first_unit.map(str::to_owned))
            .expect("valid secondary stat"),
        WalkSecondaryStat::new(kind, 14.0, second_unit.map(str::to_owned))
            .expect("valid secondary stat"),
    ];

    let result = WalkSession::new(draft);
    assert!(matches!(
        result,
        Err(WalkValidationError::DuplicateSecondaryStatKind {
            kind: actual_kind
        }) if actual_kind == kind
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
