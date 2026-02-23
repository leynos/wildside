//! HTTP request and response DTOs for route annotations (notes and progress).
//! Defines request types like `NoteRequest` and `ProgressRequest` alongside
//! response payloads such as `RouteNoteResponse`, `RouteProgressResponse`, and
//! `RouteAnnotationsResponse`.
//! Includes parsing helpers that validate payloads, convert UUID strings into
//! domain types, and bridge the inbound HTTP adapter to domain models.

use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::{Error, RouteAnnotations, RouteNote, RouteProgress};
use crate::inbound::http::validation::{
    FieldName, missing_field_error, parse_uuid, parse_uuid_list,
};

#[derive(Debug, Deserialize)]
pub(super) struct RoutePath {
    pub(super) route_id: String,
}

/// Request payload for creating or updating a note.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NoteRequest {
    #[schema(format = "uuid")]
    pub note_id: Option<String>,
    #[schema(format = "uuid")]
    pub poi_id: Option<String>,
    pub body: Option<String>,
    pub expected_revision: Option<u32>,
}

/// Request payload for updating route progress.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProgressRequest {
    #[schema(max_items = 1_000, value_type = Vec<Uuid>)]
    pub visited_stop_ids: Option<Vec<String>>,
    pub expected_revision: Option<u32>,
}

/// Response payload for a route note.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RouteNoteResponse {
    #[schema(format = "uuid")]
    pub id: String,
    #[schema(format = "uuid")]
    pub route_id: String,
    #[schema(format = "uuid")]
    pub poi_id: Option<String>,
    pub body: String,
    #[schema(format = "date-time")]
    pub created_at: String,
    #[schema(format = "date-time")]
    pub updated_at: String,
    pub revision: u32,
}

/// Response payload for route progress.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RouteProgressResponse {
    #[schema(format = "uuid")]
    pub route_id: String,
    #[schema(max_items = 1_000, value_type = Vec<Uuid>)]
    pub visited_stop_ids: Vec<String>,
    #[schema(format = "date-time")]
    pub updated_at: String,
    pub revision: u32,
}

/// Response payload aggregating notes and progress for a route.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RouteAnnotationsResponse {
    #[schema(format = "uuid")]
    pub route_id: String,
    #[schema(max_items = 1_000)]
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

pub(super) fn parse_route_id(path: RoutePath) -> Result<Uuid, Error> {
    parse_uuid(path.route_id, FieldName::new("routeId"))
}

pub(super) fn parse_note_request(payload: NoteRequest) -> Result<ParsedNoteRequest, Error> {
    let note_id = payload
        .note_id
        .ok_or_else(|| missing_field_error(FieldName::new("noteId")))?;
    let body = payload
        .body
        .ok_or_else(|| missing_field_error(FieldName::new("body")))?;
    let poi_id = payload
        .poi_id
        .map(|value| parse_uuid(value, FieldName::new("poiId")))
        .transpose()?;

    Ok(ParsedNoteRequest {
        note_id: parse_uuid(note_id, FieldName::new("noteId"))?,
        poi_id,
        body,
        expected_revision: payload.expected_revision,
    })
}

pub(super) fn parse_progress_request(
    payload: ProgressRequest,
) -> Result<ParsedProgressRequest, Error> {
    let visited_stop_ids = payload
        .visited_stop_ids
        .ok_or_else(|| missing_field_error(FieldName::new("visitedStopIds")))?;

    if visited_stop_ids.len() > 1_000 {
        return Err(
            Error::invalid_request("visited stop ids must contain at most 1000 items")
                .with_details(json!({
                    "field": "visitedStopIds",
                    "code": "too_many_items",
                    "count": visited_stop_ids.len(),
                    "max": 1000,
                })),
        );
    }

    Ok(ParsedProgressRequest {
        visited_stop_ids: parse_uuid_list(visited_stop_ids, FieldName::new("visitedStopIds"))?,
        expected_revision: payload.expected_revision,
    })
}

#[derive(Debug)]
pub(super) struct ParsedNoteRequest {
    pub(super) note_id: Uuid,
    pub(super) poi_id: Option<Uuid>,
    pub(super) body: String,
    pub(super) expected_revision: Option<u32>,
}

#[derive(Debug)]
pub(super) struct ParsedProgressRequest {
    pub(super) visited_stop_ids: Vec<Uuid>,
    pub(super) expected_revision: Option<u32>,
}
