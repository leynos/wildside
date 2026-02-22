//! Walk session domain service.
//!
//! This service implements walk session driving ports for recording sessions and
//! reading completion projections.

use std::sync::Arc;

use async_trait::async_trait;

use crate::domain::Error;
use crate::domain::ports::{
    CreateWalkSessionRequest, CreateWalkSessionResponse, GetWalkSessionRequest,
    GetWalkSessionResponse, ListWalkCompletionSummariesRequest,
    ListWalkCompletionSummariesResponse, WalkSessionCommand, WalkSessionPayload, WalkSessionQuery,
    WalkSessionRepository, WalkSessionRepositoryError,
};

/// Walk session service implementing command and query driving ports.
#[derive(Clone)]
pub struct WalkSessionService<R> {
    walk_session_repo: Arc<R>,
}

impl<R> WalkSessionService<R> {
    /// Create a new service with the given walk session repository.
    pub fn new(walk_session_repo: Arc<R>) -> Self {
        Self { walk_session_repo }
    }
}

impl<R> WalkSessionService<R>
where
    R: WalkSessionRepository,
{
    fn map_repository_error(error: WalkSessionRepositoryError) -> Error {
        match error {
            WalkSessionRepositoryError::Connection { message } => Error::service_unavailable(
                format!("walk session repository unavailable: {message}"),
            ),
            WalkSessionRepositoryError::Query { message } => {
                Error::internal(format!("walk session repository error: {message}"))
            }
        }
    }
}

#[async_trait]
impl<R> WalkSessionCommand for WalkSessionService<R>
where
    R: WalkSessionRepository,
{
    async fn create_session(
        &self,
        request: CreateWalkSessionRequest,
    ) -> Result<CreateWalkSessionResponse, Error> {
        let session = crate::domain::WalkSession::try_from(request.session).map_err(|err| {
            Error::invalid_request(format!("invalid walk session payload: {err}"))
        })?;

        self.walk_session_repo
            .save(&session)
            .await
            .map_err(Self::map_repository_error)?;

        Ok(CreateWalkSessionResponse {
            session_id: session.id(),
            completion_summary: session.completion_summary().ok().map(Into::into),
        })
    }
}

#[async_trait]
impl<R> WalkSessionQuery for WalkSessionService<R>
where
    R: WalkSessionRepository,
{
    async fn get_session(
        &self,
        request: GetWalkSessionRequest,
    ) -> Result<GetWalkSessionResponse, Error> {
        let session = self
            .walk_session_repo
            .find_by_id(&request.session_id)
            .await
            .map_err(Self::map_repository_error)?
            .ok_or_else(|| {
                Error::not_found(format!("walk session {} not found", request.session_id))
            })?;

        Ok(GetWalkSessionResponse {
            session: WalkSessionPayload::from(session),
        })
    }

    async fn list_completion_summaries(
        &self,
        request: ListWalkCompletionSummariesRequest,
    ) -> Result<ListWalkCompletionSummariesResponse, Error> {
        let summaries = self
            .walk_session_repo
            .list_completion_summaries_for_user(&request.user_id)
            .await
            .map_err(Self::map_repository_error)?;

        Ok(ListWalkCompletionSummariesResponse {
            summaries: summaries.into_iter().map(Into::into).collect(),
        })
    }
}

#[cfg(test)]
#[path = "walk_session_service_tests.rs"]
mod tests;
