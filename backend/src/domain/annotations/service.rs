//! Route annotations domain services.
//!
//! This module implements the driving ports for route annotations, enforcing
//! idempotency and optimistic concurrency semantics.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::json;

use super::idempotency::{
    IdempotencyContext, IdempotentMutationParams, IdempotentMutationRequest, PayloadHashable,
};
use crate::domain::ports::{
    DeleteNoteRequest, DeleteNoteResponse, IdempotencyRepository, IdempotencyRepositoryError,
    RouteAnnotationRepository, RouteAnnotationRepositoryError, RouteAnnotationsCommand,
    UpdateProgressRequest, UpdateProgressResponse, UpsertNoteRequest, UpsertNoteResponse,
};
use crate::domain::{Error, IdempotencyLookupResult};

type CommandFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'a>>;

/// Bundles response handling closures to keep argument counts low.
struct ResponseSpec<Build, Mark> {
    build_response: Build,
    mark_replayed: Mark,
}

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

    fn mark_replayed<T, Mark>(mut response: T, mark_replayed: Mark) -> T
    where
        Mark: Fn(&mut T),
    {
        mark_replayed(&mut response);
        response
    }

    async fn handle_duplicate_key_race<T, Mark>(
        &self,
        context: &IdempotencyContext,
        mark_replayed: Mark,
    ) -> Result<T, Error>
    where
        T: DeserializeOwned,
        Mark: Fn(&mut T) + Copy,
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
                Ok(Self::mark_replayed(response, mark_replayed))
            }
            IdempotencyLookupResult::ConflictingPayload(_) => Err(Error::conflict(
                "idempotency key already used with different payload",
            )),
            IdempotencyLookupResult::NotFound => Err(Error::internal(
                "idempotency record disappeared during race resolution",
            )),
        }
    }

    async fn handle_idempotent<T, F, Fut, Mark>(
        &self,
        context: IdempotencyContext,
        operation: F,
        mark_replayed: Mark,
    ) -> Result<T, Error>
    where
        T: DeserializeOwned + Serialize,
        Mark: Fn(&mut T) + Copy,
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
                        self.handle_duplicate_key_race(&context, mark_replayed)
                            .await
                    }
                    Err(err) => Err(Self::map_idempotency_error(err)),
                }
            }
            IdempotencyLookupResult::MatchingPayload(record) => {
                let response = Self::deserialize_response(record.response_snapshot)?;
                Ok(Self::mark_replayed(response, mark_replayed))
            }
            IdempotencyLookupResult::ConflictingPayload(_) => Err(Error::conflict(
                "idempotency key already used with different payload",
            )),
        }
    }

    async fn execute_idempotent_mutation<Req, Res, F, Fut, Mark>(
        &self,
        params: IdempotentMutationParams<'_, Req>,
        mark_replayed: Mark,
        operation: F,
    ) -> Result<Res, Error>
    where
        Req: PayloadHashable,
        Res: DeserializeOwned + Serialize,
        Mark: Fn(&mut Res) + Copy,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Res, Error>>,
    {
        let Some(idempotency_key) = params.idempotency_key else {
            return operation().await;
        };

        let context = IdempotencyContext::new(
            idempotency_key,
            params.user_id.clone(),
            params.mutation_type,
            params.request.compute_payload_hash(),
        );
        self.handle_idempotent(context, operation, mark_replayed)
            .await
    }

    async fn execute_command<Req, Res, Item, Build, Op, Mark>(
        &self,
        request: Req,
        operation: Op,
        response_spec: ResponseSpec<Build, Mark>,
    ) -> Result<Res, Error>
    where
        Req: IdempotentMutationRequest,
        Res: DeserializeOwned + Serialize,
        Op: for<'a> Fn(&'a Self, &'a Req) -> CommandFuture<'a, Item>,
        Build: FnOnce(Item) -> Res,
        Mark: Fn(&mut Res) + Copy,
    {
        let ResponseSpec {
            build_response,
            mark_replayed,
        } = response_spec;
        self.execute_idempotent_mutation(
            IdempotentMutationParams {
                request: &request,
                user_id: request.user_id(),
                mutation_type: request.mutation_type(),
                idempotency_key: request.idempotency_key(),
            },
            mark_replayed,
            || {
                let future = operation(self, &request);
                Box::pin(async move {
                    let item = future.await?;
                    Ok(build_response(item))
                })
            },
        )
        .await
    }
}

#[async_trait]
impl<R, I> RouteAnnotationsCommand for RouteAnnotationsService<R, I>
where
    R: RouteAnnotationRepository,
    I: IdempotencyRepository,
{
    async fn upsert_note(&self, request: UpsertNoteRequest) -> Result<UpsertNoteResponse, Error> {
        self.execute_command(
            request,
            |service: &Self, request: &UpsertNoteRequest| {
                Box::pin(async move { service.perform_upsert_note(request).await })
            },
            ResponseSpec {
                build_response: |note| UpsertNoteResponse {
                    note,
                    replayed: false,
                },
                mark_replayed: |response: &mut UpsertNoteResponse| {
                    response.replayed = true;
                },
            },
        )
        .await
    }

    async fn delete_note(&self, request: DeleteNoteRequest) -> Result<DeleteNoteResponse, Error> {
        self.execute_command(
            request,
            |service: &Self, request: &DeleteNoteRequest| {
                Box::pin(async move { service.perform_delete_note(request).await })
            },
            ResponseSpec {
                build_response: |deleted| DeleteNoteResponse {
                    deleted,
                    replayed: false,
                },
                mark_replayed: |response: &mut DeleteNoteResponse| {
                    response.replayed = true;
                },
            },
        )
        .await
    }

    async fn update_progress(
        &self,
        request: UpdateProgressRequest,
    ) -> Result<UpdateProgressResponse, Error> {
        self.execute_command(
            request,
            |service: &Self, request: &UpdateProgressRequest| {
                Box::pin(async move { service.perform_update_progress(request).await })
            },
            ResponseSpec {
                build_response: |progress| UpdateProgressResponse {
                    progress,
                    replayed: false,
                },
                mark_replayed: |response: &mut UpdateProgressResponse| {
                    response.replayed = true;
                },
            },
        )
        .await
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod service_tests;
