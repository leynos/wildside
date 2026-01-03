//! Route annotations HTTP handlers.
//!
//! ```text
//! GET /api/v1/routes/{route_id}/annotations
//! POST /api/v1/routes/{route_id}/notes
//! PUT /api/v1/routes/{route_id}/progress
//! ```

use actix_web::{HttpRequest, get, post, put, web};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::ports::{UpdateProgressRequest, UpsertNoteRequest};
use crate::domain::{Error, RouteAnnotations, RouteNote, RouteProgress};
use crate::inbound::http::ApiResult;
use crate::inbound::http::idempotency::{extract_idempotency_key, map_idempotency_key_error};
use crate::inbound::http::schemas::ErrorSchema;
use crate::inbound::http::session::SessionContext;
use crate::inbound::http::state::HttpState;

#[derive(Debug, Deserialize)]
struct RoutePath {
    route_id: String,
}

/// Request payload for creating or updating a note.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NoteRequest {
    pub note_id: Option<String>,
    pub poi_id: Option<String>,
    pub body: Option<String>,
    pub expected_revision: Option<u32>,
}

/// Request payload for updating route progress.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProgressRequest {
    pub visited_stop_ids: Option<Vec<String>>,
    pub expected_revision: Option<u32>,
}

/// Response payload for a route note.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RouteNoteResponse {
    pub id: String,
    pub route_id: String,
    pub poi_id: Option<String>,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
    pub revision: u32,
}

/// Response payload for route progress.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RouteProgressResponse {
    pub route_id: String,
    pub visited_stop_ids: Vec<String>,
    pub updated_at: String,
    pub revision: u32,
}

/// Response payload aggregating notes and progress for a route.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RouteAnnotationsResponse {
    pub route_id: String,
    pub notes: Vec<RouteNoteResponse>,
    pub progress: Option<RouteProgressResponse>,
}

impl From<RouteNote> for RouteNoteResponse {
    fn from(note: RouteNote) -> Self {
        Self {
            id: note.id.to_string(),
            route_id: note.route_id.to_string(),
            poi_id: note.poi_id.map(|poi| poi.to_string()),
            body: note.body,
            created_at: note.created_at.to_rfc3339(),
            updated_at: note.updated_at.to_rfc3339(),
            revision: note.revision,
        }
    }
}

impl From<RouteProgress> for RouteProgressResponse {
    fn from(progress: RouteProgress) -> Self {
        Self {
            route_id: progress.route_id.to_string(),
            visited_stop_ids: progress
                .visited_stop_ids()
                .iter()
                .map(|id| id.to_string())
                .collect(),
            updated_at: progress.updated_at.to_rfc3339(),
            revision: progress.revision,
        }
    }
}

impl From<RouteAnnotations> for RouteAnnotationsResponse {
    fn from(annotations: RouteAnnotations) -> Self {
        Self {
            route_id: annotations.route_id.to_string(),
            notes: annotations
                .notes
                .into_iter()
                .map(RouteNoteResponse::from)
                .collect(),
            progress: annotations.progress.map(RouteProgressResponse::from),
        }
    }
}

fn missing_field_error(field: &str) -> Error {
    Error::invalid_request(format!("missing required field: {field}")).with_details(json!({
        "field": field,
        "code": "missing_field",
    }))
}

fn invalid_uuid_error(field: &str, value: &str) -> Error {
    Error::invalid_request(format!("{field} must be a valid UUID")).with_details(json!({
        "field": field,
        "value": value,
        "code": "invalid_uuid",
    }))
}

fn invalid_uuid_index_error(field: &str, index: usize, value: &str) -> Error {
    Error::invalid_request(format!("{field} must contain valid UUIDs")).with_details(json!({
        "field": field,
        "index": index,
        "value": value,
        "code": "invalid_uuid",
    }))
}

fn parse_route_id(path: RoutePath) -> Result<Uuid, Error> {
    Uuid::parse_str(&path.route_id).map_err(|_| invalid_uuid_error("routeId", &path.route_id))
}

fn parse_uuid(value: String, field: &str) -> Result<Uuid, Error> {
    Uuid::parse_str(&value).map_err(|_| invalid_uuid_error(field, &value))
}

fn parse_uuid_list(values: Vec<String>, field: &str) -> Result<Vec<Uuid>, Error> {
    values
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            Uuid::parse_str(&value).map_err(|_| invalid_uuid_index_error(field, index, &value))
        })
        .collect()
}

fn route_id_from_request(request: &HttpRequest) -> Result<Uuid, Error> {
    let route_id = request
        .match_info()
        .get("route_id")
        .ok_or_else(|| missing_field_error("routeId"))?
        .to_owned();
    parse_route_id(RoutePath { route_id })
}

fn parse_note_request(payload: NoteRequest) -> Result<ParsedNoteRequest, Error> {
    let note_id = payload
        .note_id
        .ok_or_else(|| missing_field_error("noteId"))?;
    let body = payload.body.ok_or_else(|| missing_field_error("body"))?;
    let poi_id = payload
        .poi_id
        .map(|value| parse_uuid(value, "poiId"))
        .transpose()?;

    Ok(ParsedNoteRequest {
        note_id: parse_uuid(note_id, "noteId")?,
        poi_id,
        body,
        expected_revision: payload.expected_revision,
    })
}

fn parse_progress_request(payload: ProgressRequest) -> Result<ParsedProgressRequest, Error> {
    let visited_stop_ids = payload
        .visited_stop_ids
        .ok_or_else(|| missing_field_error("visitedStopIds"))?;

    Ok(ParsedProgressRequest {
        visited_stop_ids: parse_uuid_list(visited_stop_ids, "visitedStopIds")?,
        expected_revision: payload.expected_revision,
    })
}

#[derive(Debug)]
struct ParsedNoteRequest {
    note_id: Uuid,
    poi_id: Option<Uuid>,
    body: String,
    expected_revision: Option<u32>,
}

#[derive(Debug)]
struct ParsedProgressRequest {
    visited_stop_ids: Vec<Uuid>,
    expected_revision: Option<u32>,
}

/// Fetch notes and progress for a route.
#[utoipa::path(
    get,
    path = "/api/v1/routes/{route_id}/annotations",
    params(
        ("route_id" = String, Path, description = "Route identifier")
    ),
    responses(
        (status = 200, description = "Route annotations", body = RouteAnnotationsResponse),
        (status = 400, description = "Invalid request", body = ErrorSchema),
        (status = 401, description = "Unauthorised", body = ErrorSchema),
        (status = 404, description = "Not found", body = ErrorSchema),
        (status = 500, description = "Internal server error", body = ErrorSchema)
    ),
    tags = ["routes"],
    operation_id = "getRouteAnnotations"
)]
#[get("/routes/{route_id}/annotations")]
pub async fn get_annotations(
    state: web::Data<HttpState>,
    session: SessionContext,
    path: web::Path<RoutePath>,
) -> ApiResult<web::Json<RouteAnnotationsResponse>> {
    let user_id = session.require_user_id()?;
    let route_id = parse_route_id(path.into_inner())?;
    let annotations = state
        .route_annotations_query
        .fetch_annotations(route_id, &user_id)
        .await?;
    Ok(web::Json(RouteAnnotationsResponse::from(annotations)))
}

/// Create or update a note for the route.
#[utoipa::path(
    post,
    path = "/api/v1/routes/{route_id}/notes",
    request_body = NoteRequest,
    params(
        ("route_id" = String, Path, description = "Route identifier"),
        ("Idempotency-Key" = Option<String>, Header, description = "UUID for idempotent requests")
    ),
    responses(
        (status = 200, description = "Upserted note", body = RouteNoteResponse),
        (status = 400, description = "Invalid request", body = ErrorSchema),
        (status = 401, description = "Unauthorised", body = ErrorSchema),
        (status = 409, description = "Conflict", body = ErrorSchema),
        (status = 503, description = "Service unavailable", body = ErrorSchema)
    ),
    tags = ["routes"],
    operation_id = "upsertRouteNote"
)]
#[post("/routes/{route_id}/notes")]
pub async fn upsert_note(
    state: web::Data<HttpState>,
    session: SessionContext,
    request: HttpRequest,
    payload: web::Json<NoteRequest>,
) -> ApiResult<web::Json<RouteNoteResponse>> {
    let user_id = session.require_user_id()?;
    let route_id = route_id_from_request(&request)?;
    let idempotency_key =
        extract_idempotency_key(request.headers()).map_err(map_idempotency_key_error)?;
    let parsed = parse_note_request(payload.into_inner())?;

    let response = state
        .route_annotations
        .upsert_note(UpsertNoteRequest {
            note_id: parsed.note_id,
            route_id,
            poi_id: parsed.poi_id,
            user_id,
            body: parsed.body,
            expected_revision: parsed.expected_revision,
            idempotency_key,
        })
        .await?;

    Ok(web::Json(RouteNoteResponse::from(response.note)))
}

/// Update route progress for the authenticated user.
#[utoipa::path(
    put,
    path = "/api/v1/routes/{route_id}/progress",
    request_body = ProgressRequest,
    params(
        ("route_id" = String, Path, description = "Route identifier"),
        ("Idempotency-Key" = Option<String>, Header, description = "UUID for idempotent requests")
    ),
    responses(
        (status = 200, description = "Updated progress", body = RouteProgressResponse),
        (status = 400, description = "Invalid request", body = ErrorSchema),
        (status = 401, description = "Unauthorised", body = ErrorSchema),
        (status = 409, description = "Conflict", body = ErrorSchema),
        (status = 503, description = "Service unavailable", body = ErrorSchema)
    ),
    tags = ["routes"],
    operation_id = "updateRouteProgress"
)]
#[put("/routes/{route_id}/progress")]
pub async fn update_progress(
    state: web::Data<HttpState>,
    session: SessionContext,
    request: HttpRequest,
    payload: web::Json<ProgressRequest>,
) -> ApiResult<web::Json<RouteProgressResponse>> {
    let user_id = session.require_user_id()?;
    let route_id = route_id_from_request(&request)?;
    let idempotency_key =
        extract_idempotency_key(request.headers()).map_err(map_idempotency_key_error)?;
    let parsed = parse_progress_request(payload.into_inner())?;

    let response = state
        .route_annotations
        .update_progress(UpdateProgressRequest {
            route_id,
            user_id,
            visited_stop_ids: parsed.visited_stop_ids,
            expected_revision: parsed.expected_revision,
            idempotency_key,
        })
        .await?;

    Ok(web::Json(RouteProgressResponse::from(response.progress)))
}

#[cfg(test)]
#[path = "annotations_tests.rs"]
mod tests;
