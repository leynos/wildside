//! Helper types and functions for route annotation service tests.

use std::future::Future;

use super::{RouteAnnotationsService, make_service_with_idempotency};
use crate::domain::annotations::idempotency::PayloadHashable;
use crate::domain::ports::{
    DeleteNoteRequest, DeleteNoteResponse, MockIdempotencyRepository,
    MockRouteAnnotationRepository, RouteAnnotationsCommand, UpdateProgressRequest,
    UpdateProgressResponse, UpsertNoteRequest, UpsertNoteResponse,
};
use crate::domain::{
    IdempotencyKey, IdempotencyLookupQuery, IdempotencyLookupResult, IdempotencyRecord,
    MutationType, RouteNote, RouteNoteContent, RouteProgress, UserId,
};
use chrono::Utc;
use uuid::Uuid;

pub(super) struct IdempotencyReplaySpec {
    pub(super) idempotency_key: IdempotencyKey,
    pub(super) user_id: UserId,
    pub(super) mutation_type: MutationType,
    pub(super) payload_hash: crate::domain::PayloadHash,
}

pub(super) struct ReplayRequest<Req, Res> {
    pub(super) request: Req,
    pub(super) spec: IdempotencyReplaySpec,
    pub(super) response: Res,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ReplayCase {
    Note,
    Progress,
    Delete,
}

impl ReplayCase {
    pub(super) async fn assert_replay(self) {
        match self {
            ReplayCase::Note => Self::assert_note_replay().await,
            ReplayCase::Progress => Self::assert_progress_replay().await,
            ReplayCase::Delete => Self::assert_delete_replay().await,
        }
    }

    pub(super) async fn assert_note_replay() {
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
        let payload_hash = request.compute_payload_hash().expect("payload hash");
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
            |service, request| async move { service.upsert_note(request).await },
            |response| {
                assert_eq!(response.note.id, note_id);
                assert!(response.replayed);
            },
        )
        .await;
    }

    pub(super) async fn assert_progress_replay() {
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
        let payload_hash = request.compute_payload_hash().expect("payload hash");
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
            |service, request| async move { service.update_progress(request).await },
            |response| {
                assert_eq!(response.progress.route_id, route_id);
                assert!(response.replayed);
            },
        )
        .await;
    }

    pub(super) async fn assert_delete_replay() {
        let user_id = UserId::random();
        let idempotency_key = IdempotencyKey::random();
        let note_id = Uuid::new_v4();
        let request = DeleteNoteRequest {
            note_id,
            user_id: user_id.clone(),
            idempotency_key: Some(idempotency_key.clone()),
        };
        let payload_hash = request.compute_payload_hash().expect("payload hash");
        let response = DeleteNoteResponse {
            deleted: true,
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
                repo.expect_delete_note().times(0);
            },
            |service, request| async move { service.delete_note(request).await },
            |response| {
                assert!(response.deleted);
                assert!(response.replayed);
            },
        )
        .await;
    }
}

pub(super) fn mock_idempotency_replay<Res>(
    spec: IdempotencyReplaySpec,
    response: Res,
) -> MockIdempotencyRepository
where
    Res: serde::Serialize + 'static,
{
    let response_snapshot = match serde_json::to_value(response) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            panic!("response snapshot failed: {error}");
        }
    };
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

pub(super) async fn assert_replay_for_request<Req, Res, RepoFn, CallFn, CallFut, AssertFn>(
    request_spec: ReplayRequest<Req, Res>,
    setup_repo: RepoFn,
    call_service: CallFn,
    assert_response: AssertFn,
) where
    Res: serde::Serialize + Clone + 'static,
    RepoFn: FnOnce(&mut MockRouteAnnotationRepository),
    CallFn: FnOnce(
        RouteAnnotationsService<MockRouteAnnotationRepository, MockIdempotencyRepository>,
        Req,
    ) -> CallFut,
    CallFut: Future<Output = Result<Res, crate::domain::Error>>,
    AssertFn: FnOnce(Res),
{
    let mut repo = MockRouteAnnotationRepository::new();
    setup_repo(&mut repo);

    let idempotency_repo =
        mock_idempotency_replay(request_spec.spec, request_spec.response.clone());
    let service = make_service_with_idempotency(repo, idempotency_repo);

    let response = match call_service(service, request_spec.request).await {
        Ok(response) => response,
        Err(error) => {
            panic!("cached response failed: {error}");
        }
    };
    assert_response(response);
}
