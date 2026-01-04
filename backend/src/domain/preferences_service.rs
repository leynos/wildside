//! User preferences domain services.
//!
//! This module implements the driving ports for user preferences, ensuring
//! idempotency and optimistic concurrency semantics are enforced consistently.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;

use crate::domain::ports::{
    IdempotencyRepository, IdempotencyRepositoryError, UpdatePreferencesRequest,
    UpdatePreferencesResponse, UserPreferencesCommand, UserPreferencesQuery,
    UserPreferencesRepository, UserPreferencesRepositoryError,
};
use crate::domain::{
    Error, IdempotencyKey, IdempotencyLookupQuery, IdempotencyLookupResult, IdempotencyRecord,
    MutationType, PayloadHash, UserId, UserPreferences, canonicalize_and_hash,
};

/// User preferences service implementing the driving ports.
#[derive(Clone)]
pub struct UserPreferencesService<P, I> {
    preferences_repo: Arc<P>,
    idempotency_repo: Arc<I>,
}

impl<P, I> UserPreferencesService<P, I> {
    /// Create a new service with the given repositories.
    pub fn new(preferences_repo: Arc<P>, idempotency_repo: Arc<I>) -> Self {
        Self {
            preferences_repo,
            idempotency_repo,
        }
    }
}

impl<P, I> UserPreferencesService<P, I>
where
    P: UserPreferencesRepository,
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

    fn map_preferences_error(error: UserPreferencesRepositoryError) -> Error {
        match error {
            UserPreferencesRepositoryError::Connection { message } => {
                Error::service_unavailable(format!("preferences repository unavailable: {message}"))
            }
            UserPreferencesRepositoryError::Query { message } => {
                Error::internal(format!("preferences repository error: {message}"))
            }
            UserPreferencesRepositoryError::RevisionMismatch { expected, actual } => {
                Self::revision_conflict(Some(expected), actual)
            }
        }
    }

    fn revision_conflict(expected: Option<u32>, actual: u32) -> Error {
        Error::conflict("revision mismatch").with_details(json!({
            "expectedRevision": expected,
            "actualRevision": actual,
            "code": "revision_mismatch",
        }))
    }

    fn preferences_payload_hash(request: &UpdatePreferencesRequest) -> PayloadHash {
        let payload = json!({
            "interestThemeIds": request.interest_theme_ids,
            "safetyToggleIds": request.safety_toggle_ids,
            "unitSystem": request.unit_system,
            "expectedRevision": request.expected_revision,
        });
        canonicalize_and_hash(&payload)
    }

    fn serialize_response(
        response: &UpdatePreferencesResponse,
    ) -> Result<serde_json::Value, Error> {
        serde_json::to_value(response)
            .map_err(|err| Error::internal(format!("failed to serialize response: {err}")))
    }

    fn deserialize_response(
        snapshot: serde_json::Value,
    ) -> Result<UpdatePreferencesResponse, Error> {
        serde_json::from_value(snapshot)
            .map_err(|err| Error::internal(format!("failed to deserialize response: {err}")))
    }

    fn mark_replayed(mut response: UpdatePreferencesResponse) -> UpdatePreferencesResponse {
        response.replayed = true;
        response
    }

    fn build_preferences(request: &UpdatePreferencesRequest, revision: u32) -> UserPreferences {
        UserPreferences::builder(request.user_id.clone())
            .interest_theme_ids(request.interest_theme_ids.clone())
            .safety_toggle_ids(request.safety_toggle_ids.clone())
            .unit_system(request.unit_system)
            .revision(revision)
            .build()
    }

    async fn fetch_or_create_defaults(&self, user_id: &UserId) -> Result<UserPreferences, Error> {
        if let Some(preferences) = self
            .preferences_repo
            .find_by_user_id(user_id)
            .await
            .map_err(Self::map_preferences_error)?
        {
            return Ok(preferences);
        }

        let defaults = UserPreferences::new_default(user_id.clone());
        match self.preferences_repo.save(&defaults, None).await {
            Ok(()) => Ok(defaults),
            Err(err) => {
                if let Some(preferences) = self
                    .preferences_repo
                    .find_by_user_id(user_id)
                    .await
                    .map_err(Self::map_preferences_error)?
                {
                    Ok(preferences)
                } else {
                    Err(Self::map_preferences_error(err))
                }
            }
        }
    }

    async fn perform_update(
        &self,
        request: &UpdatePreferencesRequest,
    ) -> Result<UserPreferences, Error> {
        let current = self
            .preferences_repo
            .find_by_user_id(&request.user_id)
            .await
            .map_err(Self::map_preferences_error)?;

        match (current, request.expected_revision) {
            (None, None) => {
                let preferences = Self::build_preferences(request, 1);
                self.preferences_repo
                    .save(&preferences, None)
                    .await
                    .map_err(Self::map_preferences_error)?;
                Ok(preferences)
            }
            (None, Some(expected)) => Err(Self::revision_conflict(Some(expected), 0)),
            (Some(existing), None) => Err(Self::revision_conflict(None, existing.revision)),
            (Some(existing), Some(expected)) => {
                if existing.revision != expected {
                    return Err(Self::revision_conflict(Some(expected), existing.revision));
                }
                let preferences = Self::build_preferences(request, expected + 1);
                self.preferences_repo
                    .save(&preferences, Some(expected))
                    .await
                    .map_err(Self::map_preferences_error)?;
                Ok(preferences)
            }
        }
    }

    async fn handle_duplicate_key_race(
        &self,
        idempotency_key: &IdempotencyKey,
        user_id: &UserId,
        payload_hash: &PayloadHash,
    ) -> Result<UpdatePreferencesResponse, Error> {
        let query = IdempotencyLookupQuery::new(
            idempotency_key.clone(),
            user_id.clone(),
            MutationType::Preferences,
            payload_hash.clone(),
        );
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
}

#[async_trait]
impl<P, I> UserPreferencesQuery for UserPreferencesService<P, I>
where
    P: UserPreferencesRepository,
    I: IdempotencyRepository,
{
    async fn fetch_preferences(&self, user_id: &UserId) -> Result<UserPreferences, Error> {
        self.fetch_or_create_defaults(user_id).await
    }
}

#[async_trait]
impl<P, I> UserPreferencesCommand for UserPreferencesService<P, I>
where
    P: UserPreferencesRepository,
    I: IdempotencyRepository,
{
    async fn update(
        &self,
        request: UpdatePreferencesRequest,
    ) -> Result<UpdatePreferencesResponse, Error> {
        let Some(idempotency_key) = request.idempotency_key.clone() else {
            let preferences = self.perform_update(&request).await?;
            return Ok(UpdatePreferencesResponse {
                preferences,
                replayed: false,
            });
        };

        let payload_hash = Self::preferences_payload_hash(&request);
        let query = IdempotencyLookupQuery::new(
            idempotency_key.clone(),
            request.user_id.clone(),
            MutationType::Preferences,
            payload_hash.clone(),
        );

        let lookup_result = self
            .idempotency_repo
            .lookup(&query)
            .await
            .map_err(Self::map_idempotency_error)?;

        match lookup_result {
            IdempotencyLookupResult::NotFound => {
                let preferences = self.perform_update(&request).await?;
                let response = UpdatePreferencesResponse {
                    preferences,
                    replayed: false,
                };
                let response_snapshot = Self::serialize_response(&response)?;
                let record = IdempotencyRecord {
                    key: idempotency_key.clone(),
                    mutation_type: MutationType::Preferences,
                    payload_hash,
                    response_snapshot,
                    user_id: request.user_id.clone(),
                    created_at: chrono::Utc::now(),
                };

                match self.idempotency_repo.store(&record).await {
                    Ok(()) => Ok(response),
                    Err(IdempotencyRepositoryError::DuplicateKey { .. }) => {
                        self.handle_duplicate_key_race(
                            &idempotency_key,
                            &request.user_id,
                            &record.payload_hash,
                        )
                        .await
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ports::{
        FixtureIdempotencyRepository, MockIdempotencyRepository, MockUserPreferencesRepository,
    };
    use crate::domain::{
        IdempotencyKey, IdempotencyLookupQuery, IdempotencyLookupResult, IdempotencyRecord,
        MutationType, UnitSystem, UserId, UserPreferences,
    };
    use chrono::Utc;
    use uuid::Uuid;

    fn make_service(
        repo: MockUserPreferencesRepository,
    ) -> UserPreferencesService<MockUserPreferencesRepository, FixtureIdempotencyRepository> {
        UserPreferencesService::new(Arc::new(repo), Arc::new(FixtureIdempotencyRepository))
    }

    #[tokio::test]
    async fn update_creates_preferences_when_missing() {
        let user_id = UserId::random();
        let mut repo = MockUserPreferencesRepository::new();

        repo.expect_find_by_user_id()
            .times(1)
            .return_once(|_| Ok(None));
        repo.expect_save().times(1).return_once(|_, _| Ok(()));

        let service = make_service(repo);
        let request = UpdatePreferencesRequest {
            user_id: user_id.clone(),
            interest_theme_ids: Vec::new(),
            safety_toggle_ids: Vec::new(),
            unit_system: UnitSystem::Metric,
            expected_revision: None,
            idempotency_key: None,
        };

        let response = service.update(request).await.expect("update succeeds");
        assert_eq!(response.preferences.user_id, user_id);
        assert_eq!(response.preferences.revision, 1);
        assert!(!response.replayed);
    }

    #[tokio::test]
    async fn update_rejects_missing_revision_when_record_exists() {
        let user_id = UserId::random();
        let existing = UserPreferences::builder(user_id.clone())
            .revision(3)
            .build();
        let mut repo = MockUserPreferencesRepository::new();

        repo.expect_find_by_user_id()
            .times(1)
            .return_once(move |_| Ok(Some(existing)));

        let service = make_service(repo);
        let request = UpdatePreferencesRequest {
            user_id,
            interest_theme_ids: Vec::new(),
            safety_toggle_ids: Vec::new(),
            unit_system: UnitSystem::Metric,
            expected_revision: None,
            idempotency_key: None,
        };

        let error = service.update(request).await.expect_err("conflict");
        assert_eq!(error.code(), crate::domain::ErrorCode::Conflict);
    }

    #[tokio::test]
    async fn update_rejects_revision_mismatch() {
        let user_id = UserId::random();
        let existing = UserPreferences::builder(user_id.clone())
            .revision(2)
            .build();
        let mut repo = MockUserPreferencesRepository::new();

        repo.expect_find_by_user_id()
            .times(1)
            .return_once(move |_| Ok(Some(existing)));

        let service = make_service(repo);
        let request = UpdatePreferencesRequest {
            user_id,
            interest_theme_ids: Vec::new(),
            safety_toggle_ids: Vec::new(),
            unit_system: UnitSystem::Metric,
            expected_revision: Some(1),
            idempotency_key: None,
        };

        let error = service.update(request).await.expect_err("conflict");
        assert_eq!(error.code(), crate::domain::ErrorCode::Conflict);
    }

    #[tokio::test]
    async fn update_returns_cached_response_for_duplicate_idempotency_key() {
        let user_id = UserId::random();
        let idempotency_key = IdempotencyKey::random();
        let request = UpdatePreferencesRequest {
            user_id: user_id.clone(),
            interest_theme_ids: vec![Uuid::new_v4()],
            safety_toggle_ids: vec![Uuid::new_v4()],
            unit_system: UnitSystem::Metric,
            expected_revision: Some(1),
            idempotency_key: Some(idempotency_key.clone()),
        };
        let payload_hash = UserPreferencesService::<
            MockUserPreferencesRepository,
            MockIdempotencyRepository,
        >::preferences_payload_hash(&request);
        let preferences = UserPreferences::builder(user_id.clone())
            .revision(2)
            .unit_system(UnitSystem::Metric)
            .build();
        let response_snapshot = serde_json::to_value(UpdatePreferencesResponse {
            preferences: preferences.clone(),
            replayed: false,
        })
        .expect("response snapshot");
        let record = IdempotencyRecord {
            key: idempotency_key.clone(),
            mutation_type: MutationType::Preferences,
            payload_hash: payload_hash.clone(),
            response_snapshot,
            user_id: user_id.clone(),
            created_at: Utc::now(),
        };

        let mut preferences_repo = MockUserPreferencesRepository::new();
        preferences_repo.expect_find_by_user_id().times(0);
        preferences_repo.expect_save().times(0);

        let mut idempotency_repo = MockIdempotencyRepository::new();
        idempotency_repo
            .expect_lookup()
            .withf(move |query: &IdempotencyLookupQuery| {
                query.key == idempotency_key
                    && query.user_id == user_id
                    && query.mutation_type == MutationType::Preferences
                    && query.payload_hash == payload_hash
            })
            .times(1)
            .return_once(move |_| Ok(IdempotencyLookupResult::MatchingPayload(record)));
        idempotency_repo.expect_store().times(0);

        let service =
            UserPreferencesService::new(Arc::new(preferences_repo), Arc::new(idempotency_repo));

        let response = service.update(request).await.expect("cached response");
        assert_eq!(response.preferences.revision, 2);
        assert!(response.replayed);
    }
}
