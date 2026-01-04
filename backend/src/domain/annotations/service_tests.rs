//! Tests for the route annotations service.

use std::sync::Arc;

use super::PayloadHashable;
use super::RouteAnnotationsService;
use crate::domain::ports::{
    FixtureIdempotencyRepository, MockIdempotencyRepository, MockRouteAnnotationRepository,
    RouteAnnotationsCommand, UpdateProgressRequest, UpdateProgressResponse, UpsertNoteRequest,
    UpsertNoteResponse,
};
use crate::domain::{
    IdempotencyKey, IdempotencyLookupQuery, IdempotencyLookupResult, IdempotencyRecord,
    MutationType, RouteNote, RouteNoteContent, RouteProgress, UserId,
};
use chrono::Utc;
use uuid::Uuid;

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
    let user_id = UserId::random();
    let idempotency_key = IdempotencyKey::random();
    let route_id = Uuid::new_v4();
    let note_id = Uuid::new_v4();
    let request = UpsertNoteRequest {
        note_id,
        route_id,
        poi_id: None,
        user_id: user_id.clone(),
        body: "cached".to_owned(),
        expected_revision: Some(1),
        idempotency_key: Some(idempotency_key.clone()),
    };
    let note = RouteNote::new(
        note_id,
        route_id,
        user_id.clone(),
        RouteNoteContent::new("cached"),
    );
    let payload_hash = request.compute_payload_hash();
    let response_snapshot = serde_json::to_value(UpsertNoteResponse {
        note: note.clone(),
        replayed: false,
    })
    .expect("response snapshot");
    let record = IdempotencyRecord {
        key: idempotency_key.clone(),
        mutation_type: MutationType::Notes,
        payload_hash: payload_hash.clone(),
        response_snapshot,
        user_id: user_id.clone(),
        created_at: Utc::now(),
    };

    let mut repo = MockRouteAnnotationRepository::new();
    repo.expect_find_note_by_id().times(0);
    repo.expect_save_note().times(0);

    let expected_key = idempotency_key.clone();
    let expected_user_id = user_id.clone();
    let expected_payload_hash = payload_hash.clone();
    let mut idempotency_repo = MockIdempotencyRepository::new();
    idempotency_repo
        .expect_lookup()
        .withf(move |query: &IdempotencyLookupQuery| {
            query.key == expected_key
                && query.user_id == expected_user_id
                && query.mutation_type == MutationType::Notes
                && query.payload_hash == expected_payload_hash
        })
        .times(1)
        .return_once(move |_| Ok(IdempotencyLookupResult::MatchingPayload(record)));
    idempotency_repo.expect_store().times(0);

    let service = make_service_with_idempotency(repo, idempotency_repo);
    let response = service.upsert_note(request).await.expect("cached response");
    assert_eq!(response.note.id, note_id);
    assert!(response.replayed);
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
    let route_id = Uuid::new_v4();
    let user_id = UserId::random();
    let idempotency_key = IdempotencyKey::random();
    let visited_stop_ids = vec![Uuid::new_v4()];
    let request = UpdateProgressRequest {
        route_id,
        user_id: user_id.clone(),
        visited_stop_ids: visited_stop_ids.clone(),
        expected_revision: Some(1),
        idempotency_key: Some(idempotency_key.clone()),
    };
    let progress = RouteProgress::builder(route_id, user_id.clone())
        .visited_stop_ids(visited_stop_ids)
        .revision(2)
        .build();
    let payload_hash = request.compute_payload_hash();
    let response_snapshot = serde_json::to_value(UpdateProgressResponse {
        progress: progress.clone(),
        replayed: false,
    })
    .expect("response snapshot");
    let record = IdempotencyRecord {
        key: idempotency_key.clone(),
        mutation_type: MutationType::Progress,
        payload_hash: payload_hash.clone(),
        response_snapshot,
        user_id: user_id.clone(),
        created_at: Utc::now(),
    };

    let mut repo = MockRouteAnnotationRepository::new();
    repo.expect_find_progress().times(0);
    repo.expect_save_progress().times(0);

    let expected_key = idempotency_key.clone();
    let expected_user_id = user_id.clone();
    let expected_payload_hash = payload_hash.clone();
    let mut idempotency_repo = MockIdempotencyRepository::new();
    idempotency_repo
        .expect_lookup()
        .withf(move |query: &IdempotencyLookupQuery| {
            query.key == expected_key
                && query.user_id == expected_user_id
                && query.mutation_type == MutationType::Progress
                && query.payload_hash == expected_payload_hash
        })
        .times(1)
        .return_once(move |_| Ok(IdempotencyLookupResult::MatchingPayload(record)));
    idempotency_repo.expect_store().times(0);

    let service = make_service_with_idempotency(repo, idempotency_repo);
    let response = service
        .update_progress(request)
        .await
        .expect("cached response");
    assert_eq!(response.progress.route_id, route_id);
    assert!(response.replayed);
}
