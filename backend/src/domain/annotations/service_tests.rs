//! Tests for the route annotations service.

use std::sync::Arc;

use super::RouteAnnotationsService;
use crate::domain::ports::{
    FixtureIdempotencyRepository, MockRouteAnnotationRepository, RouteAnnotationsCommand,
    UpdateProgressRequest, UpsertNoteRequest,
};
use crate::domain::{RouteNote, RouteNoteContent, RouteProgress, UserId};
use uuid::Uuid;

fn make_service(
    repo: MockRouteAnnotationRepository,
) -> RouteAnnotationsService<MockRouteAnnotationRepository, FixtureIdempotencyRepository> {
    RouteAnnotationsService::new(Arc::new(repo), Arc::new(FixtureIdempotencyRepository))
}

#[tokio::test]
async fn upsert_note_creates_note_when_missing() {
    let mut repo = MockRouteAnnotationRepository::new();
    repo.expect_find_note_by_id()
        .times(1)
        .return_once(|_| Ok(None));
    repo.expect_save_note().times(1).return_once(|_, _| Ok(()));

    let service = make_service(repo);
    let request = UpsertNoteRequest {
        note_id: Uuid::new_v4(),
        route_id: Uuid::new_v4(),
        poi_id: None,
        user_id: UserId::random(),
        body: "hello".to_owned(),
        expected_revision: None,
        idempotency_key: None,
    };

    let response = service.upsert_note(request).await.expect("upsert ok");
    assert_eq!(response.note.revision, 1);
    assert!(!response.replayed);
}

#[tokio::test]
async fn upsert_note_rejects_revision_mismatch() {
    let note_id = Uuid::new_v4();
    let route_id = Uuid::new_v4();
    let user_id = UserId::random();
    let existing = RouteNote::new(
        note_id,
        route_id,
        user_id.clone(),
        RouteNoteContent::new("note"),
    );
    let mut repo = MockRouteAnnotationRepository::new();
    repo.expect_find_note_by_id()
        .times(1)
        .return_once(move |_| Ok(Some(existing)));

    let service = make_service(repo);
    let request = UpsertNoteRequest {
        note_id,
        route_id,
        poi_id: None,
        user_id,
        body: "updated".to_owned(),
        expected_revision: Some(5),
        idempotency_key: None,
    };

    let error = service.upsert_note(request).await.expect_err("conflict");
    assert_eq!(error.code(), crate::domain::ErrorCode::Conflict);
}

#[tokio::test]
async fn update_progress_creates_record_when_missing() {
    let route_id = Uuid::new_v4();
    let user_id = UserId::random();
    let mut repo = MockRouteAnnotationRepository::new();
    repo.expect_find_progress()
        .times(1)
        .return_once(|_, _| Ok(None));
    repo.expect_save_progress()
        .times(1)
        .return_once(|_, _| Ok(()));

    let service = make_service(repo);
    let request = UpdateProgressRequest {
        route_id,
        user_id: user_id.clone(),
        visited_stop_ids: vec![Uuid::new_v4()],
        expected_revision: None,
        idempotency_key: None,
    };

    let response = service.update_progress(request).await.expect("update ok");
    assert_eq!(response.progress.route_id, route_id);
    assert_eq!(response.progress.user_id, user_id);
    assert_eq!(response.progress.revision, 1);
    assert!(!response.replayed);
}

#[tokio::test]
async fn update_progress_rejects_revision_mismatch() {
    let route_id = Uuid::new_v4();
    let user_id = UserId::random();
    let existing = RouteProgress::builder(route_id, user_id.clone())
        .visited_stop_ids(vec![Uuid::new_v4()])
        .revision(3)
        .build();
    let mut repo = MockRouteAnnotationRepository::new();
    repo.expect_find_progress()
        .times(1)
        .return_once(move |_, _| Ok(Some(existing)));

    let service = make_service(repo);
    let request = UpdateProgressRequest {
        route_id,
        user_id,
        visited_stop_ids: vec![],
        expected_revision: Some(1),
        idempotency_key: None,
    };

    let error = service
        .update_progress(request)
        .await
        .expect_err("conflict");
    assert_eq!(error.code(), crate::domain::ErrorCode::Conflict);
}
