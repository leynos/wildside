//! Driving port for offline bundle mutations.
//!
//! This port defines the domain-facing contract for creating/updating and
//! deleting offline bundle manifests with optional idempotency support.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{
    BoundingBox, Error, IdempotencyKey, OfflineBundle, OfflineBundleDraft, OfflineBundleKind,
    OfflineBundleStatus, OfflineValidationError, UserId, ZoomRange,
};

/// Serializable offline bundle payload for driving ports.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineBundlePayload {
    pub id: Uuid,
    pub owner_user_id: Option<UserId>,
    pub device_id: String,
    pub kind: OfflineBundleKind,
    pub route_id: Option<Uuid>,
    pub region_id: Option<String>,
    pub bounds: BoundingBox,
    pub zoom_range: ZoomRange,
    pub estimated_size_bytes: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub status: OfflineBundleStatus,
    pub progress: f32,
}

impl TryFrom<OfflineBundlePayload> for OfflineBundle {
    type Error = OfflineValidationError;

    fn try_from(value: OfflineBundlePayload) -> Result<Self, Self::Error> {
        OfflineBundle::new(OfflineBundleDraft {
            id: value.id,
            owner_user_id: value.owner_user_id,
            device_id: value.device_id,
            kind: value.kind,
            route_id: value.route_id,
            region_id: value.region_id,
            bounds: value.bounds,
            zoom_range: value.zoom_range,
            estimated_size_bytes: value.estimated_size_bytes,
            created_at: value.created_at,
            updated_at: value.updated_at,
            status: value.status,
            progress: value.progress,
        })
    }
}

impl From<OfflineBundle> for OfflineBundlePayload {
    fn from(value: OfflineBundle) -> Self {
        Self {
            id: value.id(),
            owner_user_id: value.owner_user_id().cloned(),
            device_id: value.device_id().to_owned(),
            kind: value.kind(),
            route_id: value.route_id(),
            region_id: value.region_id().map(str::to_owned),
            bounds: value.bounds(),
            zoom_range: value.zoom_range(),
            estimated_size_bytes: value.estimated_size_bytes(),
            created_at: value.created_at(),
            updated_at: value.updated_at(),
            status: value.status(),
            progress: value.progress(),
        }
    }
}

/// Request to create or update an offline bundle manifest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertOfflineBundleRequest {
    pub user_id: UserId,
    pub bundle: OfflineBundlePayload,
    pub idempotency_key: Option<IdempotencyKey>,
}

/// Response from creating or updating an offline bundle manifest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertOfflineBundleResponse {
    pub bundle: OfflineBundlePayload,
    #[serde(rename = "replayed")]
    pub is_replayed: bool,
}

/// Request to delete an offline bundle manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteOfflineBundleRequest {
    pub user_id: UserId,
    pub bundle_id: Uuid,
    pub idempotency_key: Option<IdempotencyKey>,
}

/// Response from deleting an offline bundle manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteOfflineBundleResponse {
    pub bundle_id: Uuid,
    #[serde(rename = "replayed")]
    pub is_replayed: bool,
}

/// Driving port for offline bundle mutation operations.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait OfflineBundleCommand: Send + Sync {
    /// Creates or updates an offline bundle manifest for the authenticated user.
    ///
    /// Returns `UpsertOfflineBundleResponse` on success. Returns `Error` for
    /// validation failures, ownership violations, idempotency conflicts, or
    /// persistence failures. Callers should handle `Result::Err` by mapping
    /// the domain error code to the transport/protocol boundary.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let command = backend::domain::ports::FixtureOfflineBundleCommand;
    /// let request = backend::domain::ports::UpsertOfflineBundleRequest {
    ///     user_id: backend::domain::UserId::random(),
    ///     bundle: fixture_bundle_payload(),
    ///     idempotency_key: None,
    /// };
    /// let _response = command.upsert_bundle(request).await?;
    /// # Ok::<(), backend::domain::Error>(())
    /// ```
    async fn upsert_bundle(
        &self,
        request: UpsertOfflineBundleRequest,
    ) -> Result<UpsertOfflineBundleResponse, Error>;

    /// Deletes an existing offline bundle manifest for the authenticated user.
    ///
    /// Returns `DeleteOfflineBundleResponse` on success. Returns `Error` when
    /// the bundle is missing, ownership is invalid, idempotency keys conflict,
    /// or persistence fails. Callers should inspect `Result::Err` and map it
    /// to the appropriate boundary response.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let command = backend::domain::ports::FixtureOfflineBundleCommand;
    /// let request = backend::domain::ports::DeleteOfflineBundleRequest {
    ///     user_id: backend::domain::UserId::random(),
    ///     bundle_id: uuid::Uuid::new_v4(),
    ///     idempotency_key: None,
    /// };
    /// let _response = command.delete_bundle(request).await?;
    /// # Ok::<(), backend::domain::Error>(())
    /// ```
    async fn delete_bundle(
        &self,
        request: DeleteOfflineBundleRequest,
    ) -> Result<DeleteOfflineBundleResponse, Error>;
}

/// Fixture command implementation for tests that do not need persistence.
#[derive(Debug, Default, Clone, Copy)]
pub struct FixtureOfflineBundleCommand;

#[async_trait]
impl OfflineBundleCommand for FixtureOfflineBundleCommand {
    async fn upsert_bundle(
        &self,
        request: UpsertOfflineBundleRequest,
    ) -> Result<UpsertOfflineBundleResponse, Error> {
        Ok(UpsertOfflineBundleResponse {
            bundle: request.bundle,
            is_replayed: false,
        })
    }

    async fn delete_bundle(
        &self,
        request: DeleteOfflineBundleRequest,
    ) -> Result<DeleteOfflineBundleResponse, Error> {
        Ok(DeleteOfflineBundleResponse {
            bundle_id: request.bundle_id,
            is_replayed: false,
        })
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for this module.

    use chrono::{DateTime, Utc};
    use rstest::{fixture, rstest};

    use super::*;

    fn fixture_timestamp() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-01-02T03:04:05Z")
            .expect("RFC3339 fixture timestamp")
            .with_timezone(&Utc)
    }

    #[fixture]
    fn sample_payload() -> OfflineBundlePayload {
        let timestamp = fixture_timestamp();
        OfflineBundlePayload {
            id: Uuid::new_v4(),
            owner_user_id: Some(UserId::random()),
            device_id: "fixture-device".to_owned(),
            kind: OfflineBundleKind::Route,
            route_id: Some(Uuid::new_v4()),
            region_id: None,
            bounds: BoundingBox::new(-3.2, 55.9, -3.0, 56.0).expect("valid bounds"),
            zoom_range: ZoomRange::new(11, 15).expect("valid zoom"),
            estimated_size_bytes: 1_500,
            created_at: timestamp,
            updated_at: timestamp,
            status: OfflineBundleStatus::Queued,
            progress: 0.0,
        }
    }

    #[rstest]
    fn payload_round_trip_through_domain_entity(sample_payload: OfflineBundlePayload) {
        let payload = sample_payload;

        let bundle = OfflineBundle::try_from(payload.clone()).expect("payload is valid");
        let restored = OfflineBundlePayload::from(bundle);

        assert_eq!(restored.id, payload.id);
        assert_eq!(restored.device_id, payload.device_id);
        assert_eq!(restored.kind, payload.kind);
    }

    #[rstest]
    #[tokio::test]
    async fn fixture_command_returns_input_bundle(sample_payload: OfflineBundlePayload) {
        let command = FixtureOfflineBundleCommand;
        let request = UpsertOfflineBundleRequest {
            user_id: UserId::random(),
            bundle: sample_payload,
            idempotency_key: None,
        };

        let response = command
            .upsert_bundle(request.clone())
            .await
            .expect("fixture upsert succeeds");

        assert_eq!(response.bundle.id, request.bundle.id);
        assert!(!response.is_replayed);
    }

    #[tokio::test]
    async fn fixture_delete_returns_requested_bundle_id() {
        let command = FixtureOfflineBundleCommand;
        let request = DeleteOfflineBundleRequest {
            user_id: UserId::random(),
            bundle_id: Uuid::new_v4(),
            idempotency_key: Some(IdempotencyKey::random()),
        };

        let response = command
            .delete_bundle(request.clone())
            .await
            .expect("fixture delete succeeds");

        assert_eq!(response.bundle_id, request.bundle_id);
        assert!(!response.is_replayed);
    }
}
