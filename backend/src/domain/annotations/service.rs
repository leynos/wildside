//! Route annotations domain services.
//!
//! This module implements the driving ports for route annotations, enforcing
//! idempotency and optimistic concurrency semantics.

use std::future::Future;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::json;

use crate::domain::ports::{
    DeleteNoteRequest, DeleteNoteResponse, IdempotencyRepository, IdempotencyRepositoryError,
    RouteAnnotationRepository, RouteAnnotationRepositoryError, RouteAnnotationsCommand,
    UpdateProgressRequest, UpdateProgressResponse, UpsertNoteRequest, UpsertNoteResponse,
};
use crate::domain::{
    Error, IdempotencyKey, IdempotencyLookupQuery, IdempotencyLookupResult, IdempotencyRecord,
    MutationType, PayloadHash, UserId, canonicalize_and_hash,
};

/// Route annotations service implementing the driving ports.
#[derive(Clone)]
pub struct RouteAnnotationsService<R, I> {
    pub(super) annotations_repo: Arc<R>,
    idempotency_repo: Arc<I>,
}

impl<R, I> RouteAnnotationsService<R, I> {
    /// Create a new service with the given repositories.
    pub fn new(annotations_repo: Arc<R>, idempotency_repo: Arc<I>) -> Self {
        Self {
            annotations_repo,
            idempotency_repo,
        }
    }
}

impl<R, I> RouteAnnotationsService<R, I>
where
    R: RouteAnnotationRepository,
    I: IdempotencyRepository,
{
    fn map_idempotency_error(error: IdempotencyRepositoryError) -> Error {
        match error {
            IdempotencyRepositoryError::Connection { message } => {
                Error::service_unavailable(format!("idempotency repository unavailable: {message}"))
            }
            IdempotencyRepositoryError::Query { message } => {
                Error::internal(format!("idempotency repository error: {message}"))
            }
            IdempotencyRepositoryError::Serialization { message } => Error::internal(format!(
                "idempotency repository serialization failed: {message}"
            )),
            IdempotencyRepositoryError::DuplicateKey { message } => {
                Error::internal(format!("unexpected idempotency key conflict: {message}"))
            }
        }
    }

    pub(super) fn map_annotations_error(error: RouteAnnotationRepositoryError) -> Error {
        match error {
            RouteAnnotationRepositoryError::Connection { message } => {
                Error::service_unavailable(format!("annotation repository unavailable: {message}"))
            }
            RouteAnnotationRepositoryError::Query { message } => {
                Error::internal(format!("annotation repository error: {message}"))
            }
            RouteAnnotationRepositoryError::RevisionMismatch { expected, actual } => {
                Self::revision_conflict(Some(expected), actual)
            }
            RouteAnnotationRepositoryError::RouteNotFound { route_id } => {
                Error::not_found("route not found").with_details(json!({
                    "routeId": route_id,
                    "code": "route_not_found",
                }))
            }
        }
    }

    pub(super) fn revision_conflict(expected: Option<u32>, actual: u32) -> Error {
        Error::conflict("revision mismatch").with_details(json!({
            "expectedRevision": expected,
            "actualRevision": actual,
            "code": "revision_mismatch",
        }))
    }

    fn serialize_response<T: serde::Serialize>(response: &T) -> Result<serde_json::Value, Error> {
        serde_json::to_value(response)
            .map_err(|err| Error::internal(format!("failed to serialize response: {err}")))
    }

    fn deserialize_response<T: DeserializeOwned>(snapshot: serde_json::Value) -> Result<T, Error> {
        serde_json::from_value(snapshot)
            .map_err(|err| Error::internal(format!("failed to deserialize response: {err}")))
    }

    fn mark_replayed<T>(mut response: T) -> T
    where
        T: HasReplayFlag,
    {
        response.mark_replayed();
        response
    }

    fn note_payload_hash(request: &UpsertNoteRequest) -> PayloadHash {
        let payload = json!({
            "routeId": request.route_id,
            "noteId": request.note_id,
            "poiId": request.poi_id,
            "body": request.body,
            "expectedRevision": request.expected_revision,
        });
        canonicalize_and_hash(&payload)
    }

    fn progress_payload_hash(request: &UpdateProgressRequest) -> PayloadHash {
        let payload = json!({
            "routeId": request.route_id,
            "visitedStopIds": request.visited_stop_ids,
            "expectedRevision": request.expected_revision,
        });
        canonicalize_and_hash(&payload)
    }

    fn delete_payload_hash(request: &DeleteNoteRequest) -> PayloadHash {
        let payload = json!({
            "noteId": request.note_id,
        });
        canonicalize_and_hash(&payload)
    }

    async fn handle_duplicate_key_race<T>(&self, context: &IdempotencyContext) -> Result<T, Error>
    where
        T: DeserializeOwned + HasReplayFlag,
    {
        let query = context.lookup_query();
        let retry_result = self
            .idempotency_repo
            .lookup(&query)
            .await
            .map_err(Self::map_idempotency_error)?;

        match retry_result {
            IdempotencyLookupResult::MatchingPayload(record) => {
                let response = Self::deserialize_response(record.response_snapshot)?;
                Ok(Self::mark_replayed(response))
            }
            IdempotencyLookupResult::ConflictingPayload(_) => Err(Error::conflict(
                "idempotency key already used with different payload",
            )),
            IdempotencyLookupResult::NotFound => Err(Error::internal(
                "idempotency record disappeared during race resolution",
            )),
        }
    }

    async fn handle_idempotent<T, F, Fut>(
        &self,
        context: IdempotencyContext,
        operation: F,
    ) -> Result<T, Error>
    where
        T: DeserializeOwned + Serialize + HasReplayFlag,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, Error>>,
    {
        let query = context.lookup_query();
        let lookup_result = self
            .idempotency_repo
            .lookup(&query)
            .await
            .map_err(Self::map_idempotency_error)?;

        match lookup_result {
            IdempotencyLookupResult::NotFound => {
                let response = operation().await?;
                let response_snapshot = Self::serialize_response(&response)?;
                let record = context.record(response_snapshot);

                match self.idempotency_repo.store(&record).await {
                    Ok(()) => Ok(response),
                    Err(IdempotencyRepositoryError::DuplicateKey { .. }) => {
                        self.handle_duplicate_key_race(&context).await
                    }
                    Err(err) => Err(Self::map_idempotency_error(err)),
                }
            }
            IdempotencyLookupResult::MatchingPayload(record) => {
                let response = Self::deserialize_response(record.response_snapshot)?;
                Ok(Self::mark_replayed(response))
            }
            IdempotencyLookupResult::ConflictingPayload(_) => Err(Error::conflict(
                "idempotency key already used with different payload",
            )),
        }
    }
}

#[derive(Debug, Clone)]
struct IdempotencyContext {
    key: IdempotencyKey,
    user_id: UserId,
    mutation_type: MutationType,
    payload_hash: PayloadHash,
}

impl IdempotencyContext {
    fn new(
        key: IdempotencyKey,
        user_id: UserId,
        mutation_type: MutationType,
        payload_hash: PayloadHash,
    ) -> Self {
        Self {
            key,
            user_id,
            mutation_type,
            payload_hash,
        }
    }

    fn lookup_query(&self) -> IdempotencyLookupQuery {
        IdempotencyLookupQuery::new(
            self.key.clone(),
            self.user_id.clone(),
            self.mutation_type,
            self.payload_hash.clone(),
        )
    }

    fn record(&self, response_snapshot: serde_json::Value) -> IdempotencyRecord {
        IdempotencyRecord {
            key: self.key.clone(),
            mutation_type: self.mutation_type,
            payload_hash: self.payload_hash.clone(),
            response_snapshot,
            user_id: self.user_id.clone(),
            created_at: chrono::Utc::now(),
        }
    }
}

#[async_trait]
impl<R, I> RouteAnnotationsCommand for RouteAnnotationsService<R, I>
where
    R: RouteAnnotationRepository,
    I: IdempotencyRepository,
{
    async fn upsert_note(&self, request: UpsertNoteRequest) -> Result<UpsertNoteResponse, Error> {
        let Some(idempotency_key) = request.idempotency_key.clone() else {
            let note = self.perform_upsert_note(&request).await?;
            return Ok(UpsertNoteResponse {
                note,
                replayed: false,
            });
        };

        let payload_hash = Self::note_payload_hash(&request);
        let context = IdempotencyContext::new(
            idempotency_key,
            request.user_id.clone(),
            MutationType::Notes,
            payload_hash,
        );
        self.handle_idempotent(context, || async {
            let note = self.perform_upsert_note(&request).await?;
            Ok(UpsertNoteResponse {
                note,
                replayed: false,
            })
        })
        .await
    }

    async fn delete_note(&self, request: DeleteNoteRequest) -> Result<DeleteNoteResponse, Error> {
        let Some(idempotency_key) = request.idempotency_key.clone() else {
            let deleted = self.perform_delete_note(&request).await?;
            return Ok(DeleteNoteResponse {
                deleted,
                replayed: false,
            });
        };

        let payload_hash = Self::delete_payload_hash(&request);
        let context = IdempotencyContext::new(
            idempotency_key,
            request.user_id.clone(),
            MutationType::Notes,
            payload_hash,
        );
        self.handle_idempotent(context, || async {
            let deleted = self.perform_delete_note(&request).await?;
            Ok(DeleteNoteResponse {
                deleted,
                replayed: false,
            })
        })
        .await
    }

    async fn update_progress(
        &self,
        request: UpdateProgressRequest,
    ) -> Result<UpdateProgressResponse, Error> {
        let Some(idempotency_key) = request.idempotency_key.clone() else {
            let progress = self.perform_update_progress(&request).await?;
            return Ok(UpdateProgressResponse {
                progress,
                replayed: false,
            });
        };

        let payload_hash = Self::progress_payload_hash(&request);
        let context = IdempotencyContext::new(
            idempotency_key,
            request.user_id.clone(),
            MutationType::Progress,
            payload_hash,
        );
        self.handle_idempotent(context, || async {
            let progress = self.perform_update_progress(&request).await?;
            Ok(UpdateProgressResponse {
                progress,
                replayed: false,
            })
        })
        .await
    }
}

trait HasReplayFlag {
    fn mark_replayed(&mut self);
}

impl HasReplayFlag for UpsertNoteResponse {
    fn mark_replayed(&mut self) {
        self.replayed = true;
    }
}

impl HasReplayFlag for UpdateProgressResponse {
    fn mark_replayed(&mut self) {
        self.replayed = true;
    }
}

impl HasReplayFlag for DeleteNoteResponse {
    fn mark_replayed(&mut self) {
        self.replayed = true;
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod service_tests;
