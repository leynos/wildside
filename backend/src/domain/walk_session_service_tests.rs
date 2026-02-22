//! Tests for walk session service.

use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use super::*;
use crate::domain::ports::{MockWalkSessionRepository, WalkSessionRepositoryError};
use crate::domain::{
    WalkPrimaryStat, WalkPrimaryStatKind, WalkSecondaryStat, WalkSecondaryStatKind,
};

fn sample_create_request() -> CreateWalkSessionRequest {
    let started_at = Utc::now();
    CreateWalkSessionRequest {
        session: WalkSessionPayload {
            id: Uuid::new_v4(),
            user_id: crate::domain::UserId::random(),
            route_id: Uuid::new_v4(),
            started_at,
            ended_at: Some(started_at),
            primary_stats: vec![crate::domain::WalkPrimaryStatDraft {
                kind: WalkPrimaryStatKind::Distance,
                value: 1000.0,
            }],
            secondary_stats: vec![crate::domain::WalkSecondaryStatDraft {
                kind: WalkSecondaryStatKind::Energy,
                value: 120.0,
                unit: Some("kcal".to_owned()),
            }],
            highlighted_poi_ids: vec![Uuid::new_v4()],
        },
    }
}

#[tokio::test]
async fn create_session_persists_and_returns_stable_id() {
    let request = sample_create_request();
    let expected_session_id = request.session.id;

    let mut repo = MockWalkSessionRepository::new();
    repo.expect_save().times(1).return_once(|_| Ok(()));

    let service = WalkSessionService::new(Arc::new(repo));
    let response = service
        .create_session(request)
        .await
        .expect("create session succeeds");

    assert_eq!(response.session_id, expected_session_id);
    assert!(response.completion_summary.is_some());
}

#[tokio::test]
async fn create_session_maps_validation_error_to_invalid_request() {
    let mut request = sample_create_request();
    request.session.primary_stats = vec![crate::domain::WalkPrimaryStatDraft {
        kind: WalkPrimaryStatKind::Distance,
        value: -1.0,
    }];

    let mut repo = MockWalkSessionRepository::new();
    repo.expect_save().times(0);

    let service = WalkSessionService::new(Arc::new(repo));
    let error = service
        .create_session(request)
        .await
        .expect_err("invalid request");

    assert_eq!(error.code(), crate::domain::ErrorCode::InvalidRequest);
}

#[tokio::test]
async fn create_session_maps_connection_error_to_service_unavailable() {
    let request = sample_create_request();

    let mut repo = MockWalkSessionRepository::new();
    repo.expect_save()
        .times(1)
        .return_once(|_| Err(WalkSessionRepositoryError::connection("pool unavailable")));

    let service = WalkSessionService::new(Arc::new(repo));
    let error = service
        .create_session(request)
        .await
        .expect_err("service unavailable");

    assert_eq!(error.code(), crate::domain::ErrorCode::ServiceUnavailable);
}

#[tokio::test]
async fn get_session_returns_not_found_when_missing() {
    let mut repo = MockWalkSessionRepository::new();
    repo.expect_find_by_id().times(1).return_once(|_| Ok(None));

    let service = WalkSessionService::new(Arc::new(repo));
    let error = service
        .get_session(GetWalkSessionRequest {
            session_id: Uuid::new_v4(),
        })
        .await
        .expect_err("not found");

    assert_eq!(error.code(), crate::domain::ErrorCode::NotFound);
}

#[tokio::test]
async fn list_completion_summaries_returns_payloads() {
    let user_id = crate::domain::UserId::random();
    let started_at = Utc::now();
    let session = crate::domain::WalkSession::new(crate::domain::WalkSessionDraft {
        id: Uuid::new_v4(),
        user_id: user_id.clone(),
        route_id: Uuid::new_v4(),
        started_at,
        ended_at: Some(started_at),
        primary_stats: vec![
            WalkPrimaryStat::new(WalkPrimaryStatKind::Distance, 1000.0)
                .expect("valid primary stat"),
        ],
        secondary_stats: vec![
            WalkSecondaryStat::new(
                WalkSecondaryStatKind::Energy,
                120.0,
                Some("kcal".to_owned()),
            )
            .expect("valid secondary stat"),
        ],
        highlighted_poi_ids: vec![Uuid::new_v4()],
    })
    .expect("valid session");
    let summary = session.completion_summary().expect("completion summary");

    let mut repo = MockWalkSessionRepository::new();
    repo.expect_list_completion_summaries_for_user()
        .times(1)
        .return_once(|_| Ok(vec![summary]));

    let service = WalkSessionService::new(Arc::new(repo));
    let response = service
        .list_completion_summaries(ListWalkCompletionSummariesRequest { user_id })
        .await
        .expect("list succeeds");

    assert_eq!(response.summaries.len(), 1);
}
