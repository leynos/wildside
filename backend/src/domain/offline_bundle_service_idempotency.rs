//! Idempotency orchestration helpers for offline bundle command mutations.

use std::future::Future;
use std::time::Duration;

use serde::{Serialize, de::DeserializeOwned};

use super::*;
use crate::domain::ports::IdempotencyRepositoryError;
use crate::domain::{
    IdempotencyLookupQuery, IdempotencyLookupResult, IdempotencyRecord, MutationType,
};

enum DuplicateRaceOutcome<T> {
    Response(T),
    Retry,
}

impl<R, I> OfflineBundleCommandService<R, I>
where
    R: OfflineBundleRepository,
    I: IdempotencyRepository,
{
    const IN_PROGRESS_TIMEOUT_MESSAGE: &str =
        "idempotent request is still in progress; retry shortly";
    const FAILED_REQUEST_MESSAGE: &str =
        "idempotent request failed before completion; retry with a new idempotency key";
    const RACE_NOT_FOUND_MESSAGE: &str = "idempotency record disappeared during race resolution";
    const IDEMPOTENCY_STATE_KEY: &str = "__idempotency_state";
    const IDEMPOTENCY_STATE_IN_PROGRESS: &str = "in_progress";
    const IDEMPOTENCY_STATE_FAILED: &str = "failed";
    const DUPLICATE_RACE_MAX_RETRIES: usize = 20;
    const DUPLICATE_RACE_RETRY_DELAY: Duration = Duration::from_millis(25);

    fn in_progress_snapshot() -> serde_json::Value {
        json!({ Self::IDEMPOTENCY_STATE_KEY: Self::IDEMPOTENCY_STATE_IN_PROGRESS })
    }

    fn failed_snapshot() -> serde_json::Value {
        json!({ Self::IDEMPOTENCY_STATE_KEY: Self::IDEMPOTENCY_STATE_FAILED })
    }

    fn response_state_marker(response_snapshot: &serde_json::Value) -> Option<&str> {
        response_snapshot
            .get(Self::IDEMPOTENCY_STATE_KEY)
            .and_then(serde_json::Value::as_str)
    }

    async fn wait_for_race_retry_or_timeout(
        attempt: usize,
        timeout_error: Error,
    ) -> Result<(), Error> {
        if attempt == Self::DUPLICATE_RACE_MAX_RETRIES {
            return Err(timeout_error);
        }

        tokio::time::sleep(Self::DUPLICATE_RACE_RETRY_DELAY).await;
        Ok(())
    }

    async fn resolve_matching_payload<T, M>(
        attempt: usize,
        record: IdempotencyRecord,
        mark_replayed: &M,
    ) -> Result<DuplicateRaceOutcome<T>, Error>
    where
        T: DeserializeOwned,
        M: Fn(T) -> T,
    {
        match Self::response_state_marker(&record.response_snapshot) {
            Some(Self::IDEMPOTENCY_STATE_IN_PROGRESS) => {
                Self::wait_for_race_retry_or_timeout(
                    attempt,
                    Error::service_unavailable(Self::IN_PROGRESS_TIMEOUT_MESSAGE),
                )
                .await?;
                Ok(DuplicateRaceOutcome::Retry)
            }
            Some(Self::IDEMPOTENCY_STATE_FAILED) => {
                Err(Error::service_unavailable(Self::FAILED_REQUEST_MESSAGE))
            }
            _ => {
                let response = Self::deserialize_response(record.response_snapshot)?;
                Ok(DuplicateRaceOutcome::Response(mark_replayed(response)))
            }
        }
    }

    pub(super) async fn handle_duplicate_key_race<T, M>(
        &self,
        query: &IdempotencyLookupQuery,
        mark_replayed: &M,
    ) -> Result<T, Error>
    where
        T: DeserializeOwned,
        M: Fn(T) -> T,
    {
        for attempt in 0..=Self::DUPLICATE_RACE_MAX_RETRIES {
            let retry_result = self
                .idempotency_repo
                .lookup(query)
                .await
                .map_err(map_idempotency_error)?;

            match retry_result {
                IdempotencyLookupResult::MatchingPayload(record) => {
                    match Self::resolve_matching_payload(attempt, record, mark_replayed).await? {
                        DuplicateRaceOutcome::Response(response) => return Ok(response),
                        DuplicateRaceOutcome::Retry => continue,
                    }
                }
                IdempotencyLookupResult::ConflictingPayload(_) => {
                    return Err(Error::conflict(
                        "idempotency key already used with different payload",
                    ));
                }
                IdempotencyLookupResult::NotFound => {
                    Self::wait_for_race_retry_or_timeout(
                        attempt,
                        Error::internal(Self::RACE_NOT_FOUND_MESSAGE),
                    )
                    .await?;
                }
            }
        }

        Err(Error::internal(
            "idempotency race resolution exhausted retries",
        ))
    }

    pub(super) async fn run_idempotent_mutation<T, F, Fut, M>(
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
        let claim = IdempotencyRecord {
            key: idempotency_key.clone(),
            mutation_type: MutationType::Bundles,
            payload_hash: payload_hash.clone(),
            response_snapshot: Self::in_progress_snapshot(),
            user_id: user_id.clone(),
            created_at: self.clock.utc(),
        };

        match self.idempotency_repo.store_in_progress(&claim).await {
            Ok(()) => {}
            Err(IdempotencyRepositoryError::DuplicateKey { .. }) => {
                return self.handle_duplicate_key_race(&query, &mark_replayed).await;
            }
            Err(err) => return Err(map_idempotency_error(err)),
        }

        let response = match operation().await {
            Ok(response) => response,
            Err(err) => {
                let _ = self
                    .idempotency_repo
                    .update_response_snapshot(&query, &Self::failed_snapshot())
                    .await;
                return Err(err);
            }
        };
        let response_snapshot = Self::serialize_response(&response)?;

        self.idempotency_repo
            .update_response_snapshot(&query, &response_snapshot)
            .await
            .map_err(map_idempotency_error)?;

        Ok(response)
    }
}
