//! Tests for the route annotations service.

use std::sync::Arc;

use super::RouteAnnotationsService;
use crate::domain::ports::{
    DeleteNoteRequest, FixtureIdempotencyRepository, MockIdempotencyRepository,
    MockRouteAnnotationRepository, RouteAnnotationsCommand, UpdateProgressRequest,
    UpsertNoteRequest,
};
use crate::domain::{RouteNote, RouteNoteContent, RouteProgress, UserId};
use uuid::Uuid;

#[path = "service_test_helpers.rs"]
mod test_helpers;

use test_helpers::ReplayCase;

fn make_service(
    repo: MockRouteAnnotationRepository,
) -> RouteAnnotationsService<MockRouteAnnotationRepository, FixtureIdempotencyRepository> {
    RouteAnnotationsService::new(Arc::new(repo), Arc::new(FixtureIdempotencyRepository))
}

fn make_service_with_idempotency(
    repo: MockRouteAnnotationRepository,
    idempotency_repo: MockIdempotencyRepository,
) -> RouteAnnotationsService<MockRouteAnnotationRepository, MockIdempotencyRepository> {
    RouteAnnotationsService::new(Arc::new(repo), Arc::new(idempotency_repo))
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
async fn upsert_note_replays_cached_response_for_same_idempotency_key() {
    ReplayCase::Note.assert_replay().await;
}

#[tokio::test]
async fn delete_note_deletes_existing_note() {
    let note_id = Uuid::new_v4();
    let route_id = Uuid::new_v4();
    let user_id = UserId::random();
    let note = RouteNote::new(
        note_id,
        route_id,
        user_id.clone(),
        RouteNoteContent::new("cleanup"),
    );
    let mut repo = MockRouteAnnotationRepository::new();
    repo.expect_find_note_by_id()
        .times(1)
        .return_once(move |_| Ok(Some(note)));
    repo.expect_delete_note().times(1).return_once(|_| Ok(true));

    let service = make_service(repo);
    let request = DeleteNoteRequest {
        note_id,
        user_id,
        idempotency_key: None,
    };

    let response = service.delete_note(request).await.expect("delete ok");
    assert!(response.deleted);
    assert!(!response.replayed);
}

#[tokio::test]
async fn delete_note_returns_false_when_not_found() {
    let note_id = Uuid::new_v4();
    let user_id = UserId::random();
    let mut repo = MockRouteAnnotationRepository::new();
    repo.expect_find_note_by_id()
        .times(1)
        .return_once(|_| Ok(None));
    repo.expect_delete_note().times(0);

    let service = make_service(repo);
    let request = DeleteNoteRequest {
        note_id,
        user_id,
        idempotency_key: None,
    };

    let response = service.delete_note(request).await.expect("delete ok");
    assert!(!response.deleted);
    assert!(!response.replayed);
}

#[tokio::test]
async fn delete_note_replays_cached_response_for_same_idempotency_key() {
    ReplayCase::Delete.assert_replay().await;
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

#[tokio::test]
async fn update_progress_replays_cached_response_for_same_idempotency_key() {
    ReplayCase::Progress.assert_replay().await;
}
