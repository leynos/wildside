//! Offline bundle HTTP handlers.
//!
//! ```text
//! GET /api/v1/offline/bundles
//! POST /api/v1/offline/bundles
//! DELETE /api/v1/offline/bundles/{bundle_id}
//! ```

use std::str::FromStr;

use actix_web::{HttpRequest, delete, get, post, web};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;

use crate::domain::ports::{
    DeleteOfflineBundleRequest, ListOfflineBundlesRequest, OfflineBundlePayload,
    UpsertOfflineBundleRequest,
};
use crate::domain::{
    BoundingBox, Error, OfflineBundleKind, OfflineBundleStatus, UserId, ZoomRange,
};
use crate::inbound::http::ApiResult;
use crate::inbound::http::idempotency::{extract_idempotency_key, map_idempotency_key_error};
use crate::inbound::http::schemas::ErrorSchema;
use crate::inbound::http::session::SessionContext;
use crate::inbound::http::state::HttpState;
use crate::inbound::http::validation::{missing_field_error, parse_uuid};

/// Query parameters for listing offline bundles.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListOfflineBundlesQuery {
    pub device_id: Option<String>,
}

/// Request payload for creating or updating an offline bundle manifest.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpsertOfflineBundleRequestBody {
    pub id: String,
    pub device_id: String,
    pub kind: String,
    pub route_id: Option<String>,
    pub region_id: Option<String>,
    pub bounds: BoundsBody,
    pub zoom_range: ZoomRangeBody,
    pub estimated_size_bytes: u64,
    pub created_at: String,
    pub updated_at: String,
    pub status: String,
    pub progress: f32,
}

/// Bounds payload for offline bundle requests and responses.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BoundsBody {
    pub min_lng: f64,
    pub min_lat: f64,
    pub max_lng: f64,
    pub max_lat: f64,
}

/// Zoom payload for offline bundle requests and responses.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ZoomRangeBody {
    pub min_zoom: u8,
    pub max_zoom: u8,
}

/// Response payload for an offline bundle manifest.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OfflineBundleResponse {
    pub id: String,
    pub owner_user_id: Option<String>,
    pub device_id: String,
    pub kind: String,
    pub route_id: Option<String>,
    pub region_id: Option<String>,
    pub bounds: BoundsBody,
    pub zoom_range: ZoomRangeBody,
    pub estimated_size_bytes: u64,
    pub created_at: String,
    pub updated_at: String,
    pub status: String,
    pub progress: f32,
}

/// Response payload for listing offline bundles.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListOfflineBundlesResponseBody {
    pub bundles: Vec<OfflineBundleResponse>,
}

/// Response payload for upserting an offline bundle.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpsertOfflineBundleResponseBody {
    pub bundle_id: String,
    pub replayed: bool,
    pub bundle: OfflineBundleResponse,
}

/// Response payload for deleting an offline bundle.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteOfflineBundleResponseBody {
    pub bundle_id: String,
    pub replayed: bool,
}

#[derive(Debug, Deserialize)]
struct OfflineBundlePath {
    bundle_id: String,
}

fn parse_device_id(query: ListOfflineBundlesQuery) -> Result<String, Error> {
    let Some(device_id) = query.device_id else {
        return Err(missing_field_error("deviceId"));
    };

    let normalized = device_id.trim();
    if normalized.is_empty() {
        return Err(
            Error::invalid_request("deviceId must not be empty").with_details(json!({
                "field": "deviceId",
                "code": "invalid_device_id",
            })),
        );
    }

    Ok(normalized.to_owned())
}

fn parse_kind(kind: String) -> Result<OfflineBundleKind, Error> {
    OfflineBundleKind::from_str(kind.as_str()).map_err(|_| {
        Error::invalid_request("kind must be one of: region, route").with_details(json!({
            "field": "kind",
            "value": kind,
            "code": "invalid_kind",
        }))
    })
}

fn parse_status(status: String) -> Result<OfflineBundleStatus, Error> {
    OfflineBundleStatus::from_str(status.as_str()).map_err(|_| {
        Error::invalid_request("status must be one of: queued, downloading, complete, failed")
            .with_details(json!({
                "field": "status",
                "value": status,
                "code": "invalid_status",
            }))
    })
}

fn parse_timestamp(value: String, field: &str) -> Result<DateTime<Utc>, Error> {
    DateTime::parse_from_rfc3339(&value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|_| {
            Error::invalid_request(format!("{field} must be an RFC 3339 timestamp")).with_details(
                json!({
                    "field": field,
                    "value": value,
                    "code": "invalid_timestamp",
                }),
            )
        })
}

fn parse_optional_uuid(value: Option<String>, field: &str) -> Result<Option<uuid::Uuid>, Error> {
    value.map(|raw| parse_uuid(raw, field)).transpose()
}

fn parse_bundle_payload(
    payload: UpsertOfflineBundleRequestBody,
    user_id: UserId,
) -> Result<OfflineBundlePayload, Error> {
    Ok(OfflineBundlePayload {
        id: parse_uuid(payload.id, "id")?,
        owner_user_id: Some(user_id),
        device_id: payload.device_id,
        kind: parse_kind(payload.kind)?,
        route_id: parse_optional_uuid(payload.route_id, "routeId")?,
        region_id: payload.region_id,
        bounds: BoundingBox::new(
            payload.bounds.min_lng,
            payload.bounds.min_lat,
            payload.bounds.max_lng,
            payload.bounds.max_lat,
        )
        .map_err(|err| {
            Error::invalid_request(format!("invalid bounds: {err}")).with_details(json!({
                "field": "bounds",
                "code": "invalid_bounds",
            }))
        })?,
        zoom_range: ZoomRange::new(payload.zoom_range.min_zoom, payload.zoom_range.max_zoom)
            .map_err(|err| {
                Error::invalid_request(format!("invalid zoomRange: {err}")).with_details(json!({
                    "field": "zoomRange",
                    "code": "invalid_zoom_range",
                }))
            })?,
        estimated_size_bytes: payload.estimated_size_bytes,
        created_at: parse_timestamp(payload.created_at, "createdAt")?,
        updated_at: parse_timestamp(payload.updated_at, "updatedAt")?,
        status: parse_status(payload.status)?,
        progress: payload.progress,
    })
}

impl From<OfflineBundlePayload> for OfflineBundleResponse {
    fn from(value: OfflineBundlePayload) -> Self {
        let [min_lng, min_lat, max_lng, max_lat] = value.bounds.as_array();
        Self {
            id: value.id.to_string(),
            owner_user_id: value.owner_user_id.map(|id| id.to_string()),
            device_id: value.device_id,
            kind: value.kind.as_str().to_owned(),
            route_id: value.route_id.map(|id| id.to_string()),
            region_id: value.region_id,
            bounds: BoundsBody {
                min_lng,
                min_lat,
                max_lng,
                max_lat,
            },
            zoom_range: ZoomRangeBody {
                min_zoom: value.zoom_range.min_zoom(),
                max_zoom: value.zoom_range.max_zoom(),
            },
            estimated_size_bytes: value.estimated_size_bytes,
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
            status: value.status.as_str().to_owned(),
            progress: value.progress,
        }
    }
}

/// List offline bundle manifests for the authenticated user and device.
#[utoipa::path(
    get,
    path = "/api/v1/offline/bundles",
    params(
        ("deviceId" = String, Query, description = "Client device identifier")
    ),
    responses(
        (status = 200, description = "Offline bundles", body = ListOfflineBundlesResponseBody),
        (status = 400, description = "Invalid request", body = ErrorSchema),
        (status = 401, description = "Unauthorised", body = ErrorSchema),
        (status = 503, description = "Service unavailable", body = ErrorSchema)
    ),
    tags = ["offline"],
    operation_id = "listOfflineBundles",
    security(("SessionCookie" = []))
)]
#[get("/offline/bundles")]
pub async fn list_offline_bundles(
    state: web::Data<HttpState>,
    session: SessionContext,
    query: web::Query<ListOfflineBundlesQuery>,
) -> ApiResult<web::Json<ListOfflineBundlesResponseBody>> {
    let user_id = session.require_user_id()?;
    let device_id = parse_device_id(query.into_inner())?;
    let response = state
        .offline_bundles_query
        .list_bundles(ListOfflineBundlesRequest {
            owner_user_id: Some(user_id),
            device_id,
        })
        .await?;

    Ok(web::Json(ListOfflineBundlesResponseBody {
        bundles: response
            .bundles
            .into_iter()
            .map(OfflineBundleResponse::from)
            .collect(),
    }))
}

/// Create or update an offline bundle manifest.
#[utoipa::path(
    post,
    path = "/api/v1/offline/bundles",
    request_body = UpsertOfflineBundleRequestBody,
    params(
        ("Idempotency-Key" = Option<String>, Header, description = "UUID for idempotent requests")
    ),
    responses(
        (status = 200, description = "Bundle upserted", body = UpsertOfflineBundleResponseBody),
        (status = 400, description = "Invalid request", body = ErrorSchema),
        (status = 401, description = "Unauthorised", body = ErrorSchema),
        (status = 403, description = "Forbidden", body = ErrorSchema),
        (status = 409, description = "Conflict", body = ErrorSchema),
        (status = 503, description = "Service unavailable", body = ErrorSchema)
    ),
    tags = ["offline"],
    operation_id = "upsertOfflineBundle",
    security(("SessionCookie" = []))
)]
#[post("/offline/bundles")]
pub async fn upsert_offline_bundle(
    state: web::Data<HttpState>,
    session: SessionContext,
    request: HttpRequest,
    payload: web::Json<UpsertOfflineBundleRequestBody>,
) -> ApiResult<web::Json<UpsertOfflineBundleResponseBody>> {
    let user_id = session.require_user_id()?;
    let idempotency_key =
        extract_idempotency_key(request.headers()).map_err(map_idempotency_key_error)?;
    let bundle = parse_bundle_payload(payload.into_inner(), user_id.clone())?;

    let response = state
        .offline_bundles
        .upsert_bundle(UpsertOfflineBundleRequest {
            user_id,
            bundle,
            idempotency_key,
        })
        .await?;

    Ok(web::Json(UpsertOfflineBundleResponseBody {
        bundle_id: response.bundle.id.to_string(),
        replayed: response.replayed,
        bundle: OfflineBundleResponse::from(response.bundle),
    }))
}

/// Delete an offline bundle manifest.
#[utoipa::path(
    delete,
    path = "/api/v1/offline/bundles/{bundle_id}",
    params(
        ("bundle_id" = String, Path, description = "Offline bundle identifier"),
        ("Idempotency-Key" = Option<String>, Header, description = "UUID for idempotent requests")
    ),
    responses(
        (status = 200, description = "Bundle deleted", body = DeleteOfflineBundleResponseBody),
        (status = 400, description = "Invalid request", body = ErrorSchema),
        (status = 401, description = "Unauthorised", body = ErrorSchema),
        (status = 403, description = "Forbidden", body = ErrorSchema),
        (status = 404, description = "Not found", body = ErrorSchema),
        (status = 409, description = "Conflict", body = ErrorSchema),
        (status = 503, description = "Service unavailable", body = ErrorSchema)
    ),
    tags = ["offline"],
    operation_id = "deleteOfflineBundle",
    security(("SessionCookie" = []))
)]
#[delete("/offline/bundles/{bundle_id}")]
pub async fn delete_offline_bundle(
    state: web::Data<HttpState>,
    session: SessionContext,
    request: HttpRequest,
    path: web::Path<OfflineBundlePath>,
) -> ApiResult<web::Json<DeleteOfflineBundleResponseBody>> {
    let user_id = session.require_user_id()?;
    let idempotency_key =
        extract_idempotency_key(request.headers()).map_err(map_idempotency_key_error)?;
    let bundle_id = parse_uuid(path.into_inner().bundle_id, "bundleId")?;

    let response = state
        .offline_bundles
        .delete_bundle(DeleteOfflineBundleRequest {
            user_id,
            bundle_id,
            idempotency_key,
        })
        .await?;

    Ok(web::Json(DeleteOfflineBundleResponseBody {
        bundle_id: response.bundle_id.to_string(),
        replayed: response.replayed,
    }))
}

#[cfg(test)]
#[path = "offline_tests.rs"]
mod tests;
