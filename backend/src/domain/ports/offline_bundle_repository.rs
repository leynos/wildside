//! Port for offline bundle manifest persistence.
//!
//! Offline bundle manifests are persisted metadata only (bounds, zoom range,
//! status, and progress). Tile bytes remain outside this repository contract.

use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::{OfflineBundle, UserId};

use super::define_port_error;

define_port_error! {
    /// Errors raised by offline bundle repository adapters.
    pub enum OfflineBundleRepositoryError {
        /// Repository connection could not be established.
        Connection { message: String } =>
            "offline bundle repository connection failed: {message}",
        /// Query or mutation failed during execution.
        Query { message: String } =>
            "offline bundle repository query failed: {message}",
    }
}

/// Port for offline bundle manifest persistence and lookup.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait OfflineBundleRepository: Send + Sync {
    /// Find a bundle by its id.
    async fn find_by_id(
        &self,
        bundle_id: &Uuid,
    ) -> Result<Option<OfflineBundle>, OfflineBundleRepositoryError>;

    /// List bundles for the given owner and device.
    ///
    /// When `owner_user_id` is `None`, implementations should return anonymous
    /// device-scoped bundles only.
    async fn list_for_owner_and_device(
        &self,
        owner_user_id: Option<UserId>,
        device_id: &str,
    ) -> Result<Vec<OfflineBundle>, OfflineBundleRepositoryError>;

    /// Create or update a bundle manifest.
    async fn save(&self, bundle: &OfflineBundle) -> Result<(), OfflineBundleRepositoryError>;

    /// Delete a bundle manifest.
    ///
    /// Returns `true` when a row was deleted and `false` when the bundle did
    /// not exist.
    async fn delete(&self, bundle_id: &Uuid) -> Result<bool, OfflineBundleRepositoryError>;
}

/// Fixture implementation for tests that do not exercise bundle persistence.
#[derive(Debug, Default, Clone, Copy)]
pub struct FixtureOfflineBundleRepository;

#[async_trait]
impl OfflineBundleRepository for FixtureOfflineBundleRepository {
    async fn find_by_id(
        &self,
        _bundle_id: &Uuid,
    ) -> Result<Option<OfflineBundle>, OfflineBundleRepositoryError> {
        Ok(None)
    }

    async fn list_for_owner_and_device(
        &self,
        _owner_user_id: Option<UserId>,
        _device_id: &str,
    ) -> Result<Vec<OfflineBundle>, OfflineBundleRepositoryError> {
        Ok(Vec::new())
    }

    async fn save(&self, _bundle: &OfflineBundle) -> Result<(), OfflineBundleRepositoryError> {
        Ok(())
    }

    async fn delete(&self, _bundle_id: &Uuid) -> Result<bool, OfflineBundleRepositoryError> {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for this module.

    use chrono::Utc;
    use rstest::rstest;

    use super::*;
    use crate::domain::{
        BoundingBox, OfflineBundleDraft, OfflineBundleKind, OfflineBundleStatus, ZoomRange,
    };

    fn build_bundle() -> OfflineBundle {
        OfflineBundle::new(OfflineBundleDraft {
            id: Uuid::new_v4(),
            owner_user_id: Some(UserId::random()),
            device_id: "fixture-device".to_owned(),
            kind: OfflineBundleKind::Route,
            route_id: Some(Uuid::new_v4()),
            region_id: None,
            bounds: BoundingBox::new(-3.2, 55.9, -3.0, 56.0).expect("valid bounds"),
            zoom_range: ZoomRange::new(11, 15).expect("valid zoom"),
            estimated_size_bytes: 1_500,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            status: OfflineBundleStatus::Queued,
            progress: 0.0,
        })
        .expect("valid bundle")
    }

    #[rstest]
    #[tokio::test]
    async fn fixture_find_returns_none() {
        let repo = FixtureOfflineBundleRepository;
        let found = repo
            .find_by_id(&Uuid::new_v4())
            .await
            .expect("fixture lookup succeeds");
        assert!(found.is_none());
    }

    #[rstest]
    #[tokio::test]
    async fn fixture_list_returns_empty() {
        let repo = FixtureOfflineBundleRepository;
        let listed = repo
            .list_for_owner_and_device(Some(UserId::random()), "fixture-device")
            .await
            .expect("fixture list succeeds");
        assert!(listed.is_empty());
    }

    #[rstest]
    #[tokio::test]
    async fn fixture_save_and_delete_succeed() {
        let repo = FixtureOfflineBundleRepository;
        let bundle = build_bundle();

        repo.save(&bundle).await.expect("fixture save succeeds");
        let deleted = repo
            .delete(&bundle.id())
            .await
            .expect("fixture delete succeeds");
        assert!(!deleted);
    }

    #[rstest]
    fn query_error_formats_message() {
        let err = OfflineBundleRepositoryError::query("broken sql");
        let msg = err.to_string();
        assert!(msg.contains("broken sql"));
    }
}
