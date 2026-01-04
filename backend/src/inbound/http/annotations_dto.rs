//! Route annotations DTOs and parsing helpers.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::{Error, RouteAnnotations, RouteNote, RouteProgress};
use crate::inbound::http::validation::{missing_field_error, parse_uuid, parse_uuid_list};

#[derive(Debug, Deserialize)]
pub(super) struct RoutePath {
    pub(super) route_id: String,
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
    #[schema(max_items = 1_000)]
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
    #[schema(max_items = 1_000)]
    pub visited_stop_ids: Vec<String>,
    pub updated_at: String,
    pub revision: u32,
}

/// Response payload aggregating notes and progress for a route.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RouteAnnotationsResponse {
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
    parse_uuid(path.route_id, "routeId")
}

pub(super) fn parse_note_request(payload: NoteRequest) -> Result<ParsedNoteRequest, Error> {
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

pub(super) fn parse_progress_request(
    payload: ProgressRequest,
) -> Result<ParsedProgressRequest, Error> {
    let visited_stop_ids = payload
        .visited_stop_ids
        .ok_or_else(|| missing_field_error("visitedStopIds"))?;

    Ok(ParsedProgressRequest {
        visited_stop_ids: parse_uuid_list(visited_stop_ids, "visitedStopIds")?,
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
