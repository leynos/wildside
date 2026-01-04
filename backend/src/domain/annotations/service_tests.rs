//! Tests for the route annotations service.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use super::PayloadHashable;
use super::RouteAnnotationsService;
use crate::domain::ports::{
    FixtureIdempotencyRepository, MockIdempotencyRepository, MockRouteAnnotationRepository,
    RouteAnnotationsCommand, UpdateProgressRequest, UpdateProgressResponse, UpsertNoteRequest,
    UpsertNoteResponse,
};
use crate::domain::{
    Error, IdempotencyKey, IdempotencyLookupQuery, IdempotencyLookupResult, IdempotencyRecord,
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

type ServiceFuture<'a, Res> = Pin<Box<dyn Future<Output = Result<Res, Error>> + Send + 'a>>;

fn call_upsert_note<'a>(
    service: &'a RouteAnnotationsService<MockRouteAnnotationRepository, MockIdempotencyRepository>,
    request: UpsertNoteRequest,
) -> ServiceFuture<'a, UpsertNoteResponse> {
    Box::pin(async move { service.upsert_note(request).await })
}

fn call_update_progress<'a>(
    service: &'a RouteAnnotationsService<MockRouteAnnotationRepository, MockIdempotencyRepository>,
    request: UpdateProgressRequest,
) -> ServiceFuture<'a, UpdateProgressResponse> {
    Box::pin(async move { service.update_progress(request).await })
}

struct IdempotencyReplaySpec {
    idempotency_key: IdempotencyKey,
    user_id: UserId,
    mutation_type: MutationType,
    payload_hash: crate::domain::PayloadHash,
}

struct ReplayRequest<Req, Res> {
    request: Req,
    spec: IdempotencyReplaySpec,
    response: Res,
}

#[derive(Debug, Clone, Copy)]
enum ReplayCase {
    Note,
    Progress,
}

impl ReplayCase {
    async fn assert_replay(self) {
        match self {
            ReplayCase::Note => {
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
                let response = UpsertNoteResponse {
                    note: note.clone(),
                    replayed: false,
                };

                assert_replay_for_request(
                    ReplayRequest {
                        request,
                        spec: IdempotencyReplaySpec {
                            idempotency_key,
                            user_id,
                            mutation_type: MutationType::Notes,
                            payload_hash,
                        },
                        response,
                    },
                    |repo| {
                        repo.expect_find_note_by_id().times(0);
                        repo.expect_save_note().times(0);
                    },
                    call_upsert_note,
                    |response| {
                        assert_eq!(response.note.id, note_id);
                        assert!(response.replayed);
                    },
                )
                .await;
            }
            ReplayCase::Progress => {
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
                let response = UpdateProgressResponse {
                    progress: progress.clone(),
                    replayed: false,
                };

                assert_replay_for_request(
                    ReplayRequest {
                        request,
                        spec: IdempotencyReplaySpec {
                            idempotency_key,
                            user_id,
                            mutation_type: MutationType::Progress,
                            payload_hash,
                        },
                        response,
                    },
                    |repo| {
                        repo.expect_find_progress().times(0);
                        repo.expect_save_progress().times(0);
                    },
                    call_update_progress,
                    |response| {
                        assert_eq!(response.progress.route_id, route_id);
                        assert!(response.replayed);
                    },
                )
                .await;
            }
        }
    }
}

fn mock_idempotency_replay<Res>(
    spec: IdempotencyReplaySpec,
    response: Res,
) -> MockIdempotencyRepository
where
    Res: serde::Serialize + 'static,
{
    let response_snapshot = serde_json::to_value(response).expect("response snapshot");
    let expected_mutation_type = spec.mutation_type;
    let record = IdempotencyRecord {
        key: spec.idempotency_key.clone(),
        mutation_type: spec.mutation_type,
        payload_hash: spec.payload_hash.clone(),
        response_snapshot,
        user_id: spec.user_id.clone(),
        created_at: Utc::now(),
    };

    let expected_key = spec.idempotency_key.clone();
    let expected_user_id = spec.user_id.clone();
    let expected_payload_hash = spec.payload_hash.clone();
    let mut idempotency_repo = MockIdempotencyRepository::new();
    idempotency_repo
        .expect_lookup()
        .withf(move |query: &IdempotencyLookupQuery| {
            query.key == expected_key
                && query.user_id == expected_user_id
                && query.mutation_type == expected_mutation_type
                && query.payload_hash == expected_payload_hash
        })
        .times(1)
        .return_once(move |_| Ok(IdempotencyLookupResult::MatchingPayload(record)));
    idempotency_repo.expect_store().times(0);

    idempotency_repo
}

async fn assert_replay_for_request<Req, Res, RepoFn, CallFn, AssertFn>(
    request_spec: ReplayRequest<Req, Res>,
    setup_repo: RepoFn,
    call_service: CallFn,
    assert_response: AssertFn,
) where
    Res: serde::Serialize + Clone + 'static,
    RepoFn: FnOnce(&mut MockRouteAnnotationRepository),
    CallFn: for<'a> FnOnce(
        &'a RouteAnnotationsService<MockRouteAnnotationRepository, MockIdempotencyRepository>,
        Req,
    ) -> ServiceFuture<'a, Res>,
    AssertFn: FnOnce(Res),
{
    let mut repo = MockRouteAnnotationRepository::new();
    setup_repo(&mut repo);

    let idempotency_repo =
        mock_idempotency_replay(request_spec.spec, request_spec.response.clone());
    let service = make_service_with_idempotency(repo, idempotency_repo);

    let response = call_service(&service, request_spec.request)
        .await
        .expect("cached response");
    assert_response(response);
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
