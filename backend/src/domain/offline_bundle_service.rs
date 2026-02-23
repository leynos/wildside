//! Offline bundle domain services implementing command/query ports and idempotency orchestration.
use std::future::Future;
use std::sync::Arc;

use async_trait::async_trait;
use mockable::Clock;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::json;

use crate::domain::offline_bundle_service_support::{
    IdempotentMutationContext, map_bundle_repository_error, map_idempotency_error,
};
use crate::domain::ports::{
    DeleteOfflineBundleRequest, DeleteOfflineBundleResponse, GetOfflineBundleRequest,
    GetOfflineBundleResponse, IdempotencyRepository, IdempotencyRepositoryError,
    ListOfflineBundlesRequest, ListOfflineBundlesResponse, OfflineBundleCommand,
    OfflineBundlePayload, OfflineBundleQuery, OfflineBundleRepository, UpsertOfflineBundleRequest,
    UpsertOfflineBundleResponse,
};
use crate::domain::{
    Error, IdempotencyLookupQuery, IdempotencyLookupResult, IdempotencyRecord, MutationType,
    PayloadHash, UserId, canonicalize_and_hash, normalize_offline_device_id,
};

/// Offline bundle service implementing command driving ports.
#[derive(Clone)]
pub struct OfflineBundleCommandService<R, I> {
    bundle_repo: Arc<R>,
    idempotency_repo: Arc<I>,
    clock: Arc<dyn Clock>,
}

impl<R, I> OfflineBundleCommandService<R, I> {
    /// Create a new command service with bundle and idempotency repositories.
    /// ```rust,no_run
    /// # use std::sync::Arc;
    /// # use backend::domain::ports::{FixtureIdempotencyRepository, FixtureOfflineBundleRepository, OfflineBundleCommand};
    /// # use mockable::DefaultClock;
    /// # async fn example() -> Result<(), backend::domain::Error> {
    /// let service = backend::domain::OfflineBundleCommandService::new(
    ///     Arc::new(FixtureOfflineBundleRepository),
    ///     Arc::new(FixtureIdempotencyRepository),
    ///     Arc::new(DefaultClock),
    /// );
    /// let _ = service.upsert_bundle(todo!("construct request payload")).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(bundle_repo: Arc<R>, idempotency_repo: Arc<I>, clock: Arc<dyn Clock>) -> Self {
        Self {
            bundle_repo,
            idempotency_repo,
            clock,
        }
    }
}

impl<R, I> OfflineBundleCommandService<R, I>
where
    R: OfflineBundleRepository,
    I: IdempotencyRepository,
{
    fn serialize_response<T: Serialize>(response: &T) -> Result<serde_json::Value, Error> {
        serde_json::to_value(response)
            .map_err(|err| Error::internal(format!("failed to serialize response: {err}")))
    }

    fn deserialize_response<T: DeserializeOwned>(snapshot: serde_json::Value) -> Result<T, Error> {
        serde_json::from_value(snapshot)
            .map_err(|err| Error::internal(format!("failed to deserialize response: {err}")))
    }

    fn hash_payload<T: Serialize>(payload: &T) -> Result<PayloadHash, Error> {
        let json_payload = serde_json::to_value(payload).map_err(|err| {
            Error::internal(format!("failed to serialize idempotency payload: {err}"))
        })?;
        canonicalize_and_hash(&json_payload)
            .map_err(|err| Error::internal(format!("failed to hash idempotency payload: {err}")))
    }

    fn validate_bundle_ownership(
        bundle: &OfflineBundlePayload,
        user_id: &UserId,
    ) -> Result<(), Error> {
        match bundle.owner_user_id.as_ref() {
            Some(owner_user_id) if owner_user_id == user_id => Ok(()),
            _ => Err(Error::forbidden(
                "offline bundle owner does not match session user",
            )),
        }
    }

    async fn handle_duplicate_key_race<T, M>(
        &self,
        query: &IdempotencyLookupQuery,
        mark_replayed: &M,
    ) -> Result<T, Error>
    where
        T: DeserializeOwned,
        M: Fn(T) -> T,
    {
        let retry_result = self
            .idempotency_repo
            .lookup(query)
            .await
            .map_err(map_idempotency_error)?;

        match retry_result {
            IdempotencyLookupResult::MatchingPayload(record) => {
                let response = Self::deserialize_response(record.response_snapshot)?;
                Ok(mark_replayed(response))
            }
            IdempotencyLookupResult::ConflictingPayload(_) => Err(Error::conflict(
                "idempotency key already used with different payload",
            )),
            IdempotencyLookupResult::NotFound => Err(Error::internal(
                "idempotency record disappeared during race resolution",
            )),
        }
    }

    async fn run_idempotent_mutation<T, F, Fut, M>(
        &self,
        context: IdempotentMutationContext,
        operation: F,
        mark_replayed: M,
    ) -> Result<T, Error>
    where
        T: Serialize + DeserializeOwned,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, Error>>,
        M: Fn(T) -> T,
    {
        let IdempotentMutationContext {
            idempotency_key,
            user_id,
            payload_hash,
        } = context;

        let Some(idempotency_key) = idempotency_key else {
            return operation().await;
        };

        let query = IdempotencyLookupQuery::new(
            idempotency_key.clone(),
            user_id.clone(),
            MutationType::Bundles,
            payload_hash.clone(),
        );

        let lookup_result = self
            .idempotency_repo
            .lookup(&query)
            .await
            .map_err(map_idempotency_error)?;

        match lookup_result {
            IdempotencyLookupResult::NotFound => {
                let response = operation().await?;
                let response_snapshot = Self::serialize_response(&response)?;
                let record = IdempotencyRecord {
                    key: idempotency_key.clone(),
                    mutation_type: MutationType::Bundles,
                    payload_hash: payload_hash.clone(),
                    response_snapshot,
                    user_id: user_id.clone(),
                    created_at: self.clock.utc(),
                };

                match self.idempotency_repo.store(&record).await {
                    Ok(()) => Ok(response),
                    Err(IdempotencyRepositoryError::DuplicateKey { .. }) => {
                        self.handle_duplicate_key_race(&query, &mark_replayed).await
                    }
                    Err(err) => Err(map_idempotency_error(err)),
                }
            }
            IdempotencyLookupResult::MatchingPayload(record) => {
                let response = Self::deserialize_response(record.response_snapshot)?;
                Ok(mark_replayed(response))
            }
            IdempotencyLookupResult::ConflictingPayload(_) => Err(Error::conflict(
                "idempotency key already used with different payload",
            )),
        }
    }

    async fn persist_bundle(
        &self,
        user_id: &UserId,
        bundle_payload: OfflineBundlePayload,
    ) -> Result<UpsertOfflineBundleResponse, Error> {
        let existing = self
            .bundle_repo
            .find_by_id(&bundle_payload.id)
            .await
            .map_err(map_bundle_repository_error)?;
        if let Some(existing) = existing {
            match existing.owner_user_id() {
                Some(owner_user_id) if owner_user_id == user_id => {}
                _ => {
                    return Err(Error::forbidden(
                        "offline bundle owner does not match session user",
                    ));
                }
            }
        }
        Self::validate_bundle_ownership(&bundle_payload, user_id)?;
        let bundle = crate::domain::OfflineBundle::try_from(bundle_payload).map_err(|err| {
            Error::invalid_request(format!("invalid offline bundle payload: {err}"))
        })?;

        self.bundle_repo
            .save(&bundle)
            .await
            .map_err(map_bundle_repository_error)?;

        Ok(UpsertOfflineBundleResponse {
            bundle: OfflineBundlePayload::from(bundle),
            is_replayed: false,
        })
    }

    async fn perform_delete(
        &self,
        bundle_id: uuid::Uuid,
        requesting_user_id: UserId,
    ) -> Result<DeleteOfflineBundleResponse, Error> {
        let existing = self
            .bundle_repo
            .find_by_id(&bundle_id)
            .await
            .map_err(map_bundle_repository_error)?;
        let Some(bundle) = existing else {
            return Err(Error::not_found(format!(
                "offline bundle {} not found",
                bundle_id
            )));
        };
        match bundle.owner_user_id() {
            Some(owner_user_id) if owner_user_id == &requesting_user_id => {}
            _ => {
                return Err(Error::forbidden(
                    "offline bundle owner does not match session user",
                ));
            }
        }

        let is_deleted = self
            .bundle_repo
            .delete(&bundle_id)
            .await
            .map_err(map_bundle_repository_error)?;
        if !is_deleted {
            return Err(Error::not_found(format!(
                "offline bundle {} not found",
                bundle_id
            )));
        }

        Ok(DeleteOfflineBundleResponse {
            bundle_id,
            is_replayed: false,
        })
    }
}

#[async_trait]
impl<R, I> OfflineBundleCommand for OfflineBundleCommandService<R, I>
where
    R: OfflineBundleRepository,
    I: IdempotencyRepository,
{
    async fn upsert_bundle(
        &self,
        request: UpsertOfflineBundleRequest,
    ) -> Result<UpsertOfflineBundleResponse, Error> {
        let user_id = request.user_id;
        let bundle = request.bundle;
        let payload = bundle.clone();
        let payload_hash = Self::hash_payload(&payload)?;

        self.run_idempotent_mutation(
            IdempotentMutationContext {
                idempotency_key: request.idempotency_key,
                user_id: user_id.clone(),
                payload_hash,
            },
            || async { self.persist_bundle(&user_id, bundle).await },
            |mut response: UpsertOfflineBundleResponse| {
                response.is_replayed = true;
                response
            },
        )
        .await
    }

    async fn delete_bundle(
        &self,
        request: DeleteOfflineBundleRequest,
    ) -> Result<DeleteOfflineBundleResponse, Error> {
        let payload = json!({ "bundleId": request.bundle_id });
        let payload_hash = Self::hash_payload(&payload)?;
        let user_id = request.user_id;
        let bundle_id = request.bundle_id;

        self.run_idempotent_mutation(
            IdempotentMutationContext {
                idempotency_key: request.idempotency_key,
                user_id: user_id.clone(),
                payload_hash,
            },
            || async { self.perform_delete(bundle_id, user_id.clone()).await },
            |mut response: DeleteOfflineBundleResponse| {
                response.is_replayed = true;
                response
            },
        )
        .await
    }
}

/// Offline bundle service implementing query driving ports.
#[derive(Clone)]
pub struct OfflineBundleQueryService<R> {
    bundle_repo: Arc<R>,
}

impl<R> OfflineBundleQueryService<R> {
    /// Create a new query service with the bundle repository.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use std::sync::Arc;
    /// # use backend::domain::ports::{FixtureOfflineBundleRepository, ListOfflineBundlesRequest, OfflineBundleQuery};
    /// # async fn example() -> Result<(), backend::domain::Error> {
    /// let service = backend::domain::OfflineBundleQueryService::new(Arc::new(FixtureOfflineBundleRepository));
    /// let response = service
    ///     .list_bundles(ListOfflineBundlesRequest {
    ///         owner_user_id: Some(backend::domain::UserId::random()),
    ///         device_id: "ios-iphone-15".to_owned(),
    ///     })
    ///     .await?;
    /// assert!(response.bundles.is_empty());
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(bundle_repo: Arc<R>) -> Self {
        Self { bundle_repo }
    }
}

#[async_trait]
impl<R> OfflineBundleQuery for OfflineBundleQueryService<R>
where
    R: OfflineBundleRepository,
{
    async fn list_bundles(
        &self,
        request: ListOfflineBundlesRequest,
    ) -> Result<ListOfflineBundlesResponse, Error> {
        let device_id = normalize_offline_device_id(&request.device_id)
            .map_err(|_| Error::invalid_request("deviceId must not be empty"))?;
        let bundles = self
            .bundle_repo
            .list_for_owner_and_device(request.owner_user_id, &device_id)
            .await
            .map_err(map_bundle_repository_error)?;

        Ok(ListOfflineBundlesResponse {
            bundles: bundles
                .into_iter()
                .map(OfflineBundlePayload::from)
                .collect(),
        })
    }

    async fn get_bundle(
        &self,
        request: GetOfflineBundleRequest,
    ) -> Result<GetOfflineBundleResponse, Error> {
        let bundle = self
            .bundle_repo
            .find_by_id(&request.bundle_id)
            .await
            .map_err(map_bundle_repository_error)?
            .ok_or_else(|| {
                Error::not_found(format!("offline bundle {} not found", request.bundle_id))
            })?;

        Ok(GetOfflineBundleResponse {
            bundle: OfflineBundlePayload::from(bundle),
        })
    }
}

#[cfg(test)]
#[path = "offline_bundle_service_tests.rs"]
mod tests;
