//! Driving port for walk session read operations.
//!
//! Inbound adapters use this port to read persisted walk sessions and
//! completion summaries without depending on repository details.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{Error, UserId};

use super::walk_session_command::{WalkCompletionSummaryPayload, WalkSessionPayload};

/// Request to fetch one walk session by identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetWalkSessionRequest {
    pub session_id: Uuid,
}

/// Response for a single walk session lookup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetWalkSessionResponse {
    pub session: WalkSessionPayload,
}

/// Request to list completion summaries for a user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListWalkCompletionSummariesRequest {
    pub user_id: UserId,
}

/// Response containing completion summaries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListWalkCompletionSummariesResponse {
    pub summaries: Vec<WalkCompletionSummaryPayload>,
}

/// Driving port for walk session read operations.
///
/// # Examples
///
/// ```rust,no_run
/// # async fn example() -> Result<(), backend::domain::Error> {
/// let query = backend::domain::ports::FixtureWalkSessionQuery;
/// let request = backend::domain::ports::ListWalkCompletionSummariesRequest {
///     user_id: backend::domain::UserId::random(),
/// };
/// let response = query.list_completion_summaries(request).await?;
/// assert!(response.summaries.is_empty());
/// # Ok(())
/// # }
/// ```
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait WalkSessionQuery: Send + Sync {
    /// Fetches one persisted walk session by identifier.
    ///
    /// Accepts `GetWalkSessionRequest` and returns `GetWalkSessionResponse`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # async fn example() {
    /// let query = backend::domain::ports::FixtureWalkSessionQuery;
    /// let request = backend::domain::ports::GetWalkSessionRequest {
    ///     session_id: uuid::Uuid::new_v4(),
    /// };
    /// let result = query.get_session(request).await;
    /// assert!(result.is_err());
    /// # }
    /// ```
    async fn get_session(
        &self,
        request: GetWalkSessionRequest,
    ) -> Result<GetWalkSessionResponse, Error>;

    /// Lists completion summaries for a user.
    ///
    /// Accepts `ListWalkCompletionSummariesRequest` and returns
    /// `ListWalkCompletionSummariesResponse`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # async fn example() -> Result<(), backend::domain::Error> {
    /// let query = backend::domain::ports::FixtureWalkSessionQuery;
    /// let request = backend::domain::ports::ListWalkCompletionSummariesRequest {
    ///     user_id: backend::domain::UserId::random(),
    /// };
    /// let response = query.list_completion_summaries(request).await?;
    /// assert!(response.summaries.is_empty());
    /// # Ok(())
    /// # }
    /// ```
    async fn list_completion_summaries(
        &self,
        request: ListWalkCompletionSummariesRequest,
    ) -> Result<ListWalkCompletionSummariesResponse, Error>;
}

/// Fixture query implementation for tests that do not need persistence.
#[derive(Debug, Default, Clone, Copy)]
pub struct FixtureWalkSessionQuery;

#[async_trait]
impl WalkSessionQuery for FixtureWalkSessionQuery {
    async fn get_session(
        &self,
        request: GetWalkSessionRequest,
    ) -> Result<GetWalkSessionResponse, Error> {
        Err(Error::not_found(format!(
            "walk session {} not found",
            request.session_id
        )))
    }

    async fn list_completion_summaries(
        &self,
        _request: ListWalkCompletionSummariesRequest,
    ) -> Result<ListWalkCompletionSummariesResponse, Error> {
        Ok(ListWalkCompletionSummariesResponse {
            summaries: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for this module.

    use super::*;

    #[tokio::test]
    async fn fixture_query_returns_not_found_for_get() {
        let query = FixtureWalkSessionQuery;
        let request = GetWalkSessionRequest {
            session_id: Uuid::new_v4(),
        };

        let error = query.get_session(request).await.expect_err("not found");

        assert_eq!(error.code(), crate::domain::ErrorCode::NotFound);
    }

    #[tokio::test]
    async fn fixture_query_returns_empty_summaries() {
        let query = FixtureWalkSessionQuery;
        let request = ListWalkCompletionSummariesRequest {
            user_id: UserId::random(),
        };

        let response = query
            .list_completion_summaries(request)
            .await
            .expect("fixture list succeeds");

        assert!(response.summaries.is_empty());
    }
}
