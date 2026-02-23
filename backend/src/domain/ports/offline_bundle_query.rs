//! Driving port for offline bundle read operations.
//!
//! Inbound adapters use this port to list and fetch offline bundle manifests
//! without importing persistence details.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{Error, UserId};

use super::offline_bundle_command::OfflineBundlePayload;

/// Request to list offline bundles for an owner and device scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListOfflineBundlesRequest {
    pub owner_user_id: Option<UserId>,
    pub device_id: String,
}

/// Response containing offline bundles in the requested scope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListOfflineBundlesResponse {
    pub bundles: Vec<OfflineBundlePayload>,
}

/// Request to fetch one offline bundle by identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetOfflineBundleRequest {
    pub bundle_id: Uuid,
}

/// Response for a single offline bundle lookup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetOfflineBundleResponse {
    pub bundle: OfflineBundlePayload,
}

/// Driving port for offline bundle read operations.
///
/// # Examples
///
/// ```rust,no_run
/// # async fn example() -> Result<(), backend::domain::Error> {
/// let query = backend::domain::ports::FixtureOfflineBundleQuery;
/// let request = backend::domain::ports::ListOfflineBundlesRequest {
///     owner_user_id: Some(backend::domain::UserId::random()),
///     device_id: "device-123".to_owned(),
/// };
/// let response = query.list_bundles(request).await?;
/// assert!(response.bundles.is_empty());
/// # Ok(())
/// # }
/// ```
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait OfflineBundleQuery: Send + Sync {
    /// Lists offline bundles for the owner/device scope supplied by the caller.
    ///
    /// Accepts `ListOfflineBundlesRequest` and returns
    /// `ListOfflineBundlesResponse` containing zero or more
    /// `OfflineBundlePayload` entries.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # async fn example() -> Result<(), backend::domain::Error> {
    /// let query = backend::domain::ports::FixtureOfflineBundleQuery;
    /// let request = backend::domain::ports::ListOfflineBundlesRequest {
    ///     owner_user_id: None,
    ///     device_id: "device-123".to_owned(),
    /// };
    /// let response = query.list_bundles(request).await?;
    /// assert!(response.bundles.is_empty());
    /// # Ok(())
    /// # }
    /// ```
    async fn list_bundles(
        &self,
        request: ListOfflineBundlesRequest,
    ) -> Result<ListOfflineBundlesResponse, Error>;

    /// Fetches one offline bundle by identifier.
    ///
    /// Accepts `GetOfflineBundleRequest` and returns
    /// `GetOfflineBundleResponse` for the matching bundle.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # async fn example() {
    /// let query = backend::domain::ports::FixtureOfflineBundleQuery;
    /// let request = backend::domain::ports::GetOfflineBundleRequest {
    ///     bundle_id: uuid::Uuid::new_v4(),
    /// };
    /// let result = query.get_bundle(request).await;
    /// assert!(result.is_err());
    /// # }
    /// ```
    async fn get_bundle(
        &self,
        request: GetOfflineBundleRequest,
    ) -> Result<GetOfflineBundleResponse, Error>;
}

/// Fixture query implementation for tests that do not need persistence.
#[derive(Debug, Default, Clone, Copy)]
pub struct FixtureOfflineBundleQuery;

#[async_trait]
impl OfflineBundleQuery for FixtureOfflineBundleQuery {
    async fn list_bundles(
        &self,
        _request: ListOfflineBundlesRequest,
    ) -> Result<ListOfflineBundlesResponse, Error> {
        Ok(ListOfflineBundlesResponse {
            bundles: Vec::new(),
        })
    }

    async fn get_bundle(
        &self,
        request: GetOfflineBundleRequest,
    ) -> Result<GetOfflineBundleResponse, Error> {
        Err(Error::not_found(format!(
            "offline bundle {} not found",
            request.bundle_id
        )))
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for this module.

    use super::*;

    #[tokio::test]
    async fn fixture_query_returns_empty_list() {
        let query = FixtureOfflineBundleQuery;
        let request = ListOfflineBundlesRequest {
            owner_user_id: Some(UserId::random()),
            device_id: "fixture-device".to_owned(),
        };

        let response = query
            .list_bundles(request)
            .await
            .expect("fixture list succeeds");

        assert!(response.bundles.is_empty());
    }

    #[tokio::test]
    async fn fixture_get_returns_not_found() {
        let query = FixtureOfflineBundleQuery;
        let request = GetOfflineBundleRequest {
            bundle_id: Uuid::new_v4(),
        };

        let error = query.get_bundle(request).await.expect_err("not found");

        assert_eq!(error.code(), crate::domain::ErrorCode::NotFound);
    }
}
