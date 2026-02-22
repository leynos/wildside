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
    pub replayed: bool,
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
    pub replayed: bool,
}

/// Driving port for offline bundle mutation operations.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait OfflineBundleCommand: Send + Sync {
    async fn upsert_bundle(
        &self,
        request: UpsertOfflineBundleRequest,
    ) -> Result<UpsertOfflineBundleResponse, Error>;

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
            replayed: false,
        })
    }

    async fn delete_bundle(
        &self,
        request: DeleteOfflineBundleRequest,
    ) -> Result<DeleteOfflineBundleResponse, Error> {
        Ok(DeleteOfflineBundleResponse {
            bundle_id: request.bundle_id,
            replayed: false,
        })
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for this module.

    use chrono::Utc;
    use rstest::rstest;

    use super::*;

    fn sample_payload() -> OfflineBundlePayload {
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
            created_at: Utc::now(),
            updated_at: Utc::now(),
            status: OfflineBundleStatus::Queued,
            progress: 0.0,
        }
    }

    #[rstest]
    fn payload_round_trip_through_domain_entity() {
        let payload = sample_payload();

        let bundle = OfflineBundle::try_from(payload.clone()).expect("payload is valid");
        let restored = OfflineBundlePayload::from(bundle);

        assert_eq!(restored.id, payload.id);
        assert_eq!(restored.device_id, payload.device_id);
        assert_eq!(restored.kind, payload.kind);
    }

    #[tokio::test]
    async fn fixture_command_returns_input_bundle() {
        let command = FixtureOfflineBundleCommand;
        let request = UpsertOfflineBundleRequest {
            user_id: UserId::random(),
            bundle: sample_payload(),
            idempotency_key: None,
        };

        let response = command
            .upsert_bundle(request.clone())
            .await
            .expect("fixture upsert succeeds");

        assert_eq!(response.bundle.id, request.bundle.id);
        assert!(!response.replayed);
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
        assert!(!response.replayed);
    }
}
