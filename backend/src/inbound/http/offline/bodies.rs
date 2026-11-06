//! Request and response bodies of the offline bundle HTTP endpoints.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Query parameters for listing offline bundles.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListOfflineBundlesQuery {
    /// Device whose bundles to list; required, and normalized before lookup.
    pub device_id: Option<String>,
}

/// Request payload for creating or updating an offline bundle manifest.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpsertOfflineBundleRequestBody {
    /// Client-supplied identifier of the bundle manifest, as a UUID string.
    pub id: String,
    /// Device that stores the bundle; normalized before use.
    pub device_id: String,
    /// Bundle scope, either `region` or `route`.
    pub kind: String,
    /// Route identifier as a UUID string, set for route bundles.
    pub route_id: Option<String>,
    /// Region identifier, set for region bundles.
    pub region_id: Option<String>,
    /// Geographic extent of the area to store.
    pub bounds: BoundsBody,
    /// Inclusive zoom levels to download.
    pub zoom_range: ZoomRangeBody,
    /// Estimated download size in bytes.
    pub estimated_size_bytes: u64,
    /// Creation time as an RFC 3339 timestamp.
    pub created_at: String,
    /// Last update time as an RFC 3339 timestamp.
    pub updated_at: String,
    /// Lifecycle state: `queued`, `downloading`, `complete` or `failed`.
    pub status: String,
    /// Download progress from 0.0 to 1.0 inclusive.
    pub progress: f32,
}

/// Bounds payload for offline bundle requests and responses.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BoundsBody {
    /// Western edge longitude in degrees.
    pub min_lng: f64,
    /// Southern edge latitude in degrees.
    pub min_lat: f64,
    /// Eastern edge longitude in degrees.
    pub max_lng: f64,
    /// Northern edge latitude in degrees.
    pub max_lat: f64,
}

/// Zoom payload for offline bundle requests and responses.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ZoomRangeBody {
    /// Lowest zoom level included, inclusive.
    pub min_zoom: u8,
    /// Highest zoom level included, inclusive; must not be below `min_zoom`.
    pub max_zoom: u8,
}

/// Response payload for an offline bundle manifest.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OfflineBundleResponse {
    /// Identifier of the bundle manifest.
    pub id: String,
    /// Owning user identifier, if the bundle is tied to an account.
    pub owner_user_id: Option<String>,
    /// Device that stores the bundle.
    pub device_id: String,
    /// Bundle scope, either `region` or `route`.
    pub kind: String,
    /// Route identifier, set for route bundles.
    pub route_id: Option<String>,
    /// Region identifier, set for region bundles.
    pub region_id: Option<String>,
    /// Geographic extent of the area stored.
    pub bounds: BoundsBody,
    /// Inclusive range of zoom levels the bundle targets; a queued or downloading bundle has
    /// not necessarily fetched them all yet.
    pub zoom_range: ZoomRangeBody,
    /// Estimated download size in bytes.
    pub estimated_size_bytes: u64,
    /// Creation time as an RFC 3339 timestamp.
    pub created_at: String,
    /// Last update time as an RFC 3339 timestamp.
    pub updated_at: String,
    /// Lifecycle state: `queued`, `downloading`, `complete` or `failed`.
    pub status: String,
    /// Download progress from 0.0 to 1.0 inclusive.
    pub progress: f32,
}

/// Response payload for listing offline bundles.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListOfflineBundlesResponseBody {
    /// Bundle manifests in the requested device scope.
    pub bundles: Vec<OfflineBundleResponse>,
}

/// Response payload for upserting an offline bundle.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpsertOfflineBundleResponseBody {
    /// Identifier of the upserted bundle manifest.
    pub bundle_id: String,
    /// Whether this response replays an earlier idempotent result.
    #[serde(rename = "replayed")]
    pub is_replayed: bool,
    /// Stored manifest after the upsert.
    pub bundle: OfflineBundleResponse,
}

/// Response payload for deleting an offline bundle.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteOfflineBundleResponseBody {
    /// Identifier of the deleted bundle manifest.
    pub bundle_id: String,
    /// Whether this response replays an earlier idempotent result.
    #[serde(rename = "replayed")]
    pub is_replayed: bool,
}
