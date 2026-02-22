//! Walk session HTTP handlers.
//!
//! ```text
//! POST /api/v1/walk-sessions
//! ```

use actix_web::{post, web};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;

use crate::domain::ports::{
    CreateWalkSessionRequest, CreateWalkSessionResponse, WalkCompletionSummaryPayload,
    WalkSessionPayload,
};
use crate::domain::{
    Error, UserId, WalkPrimaryStatDraft, WalkPrimaryStatKind, WalkSecondaryStatDraft,
    WalkSecondaryStatKind,
};
use crate::inbound::http::ApiResult;
use crate::inbound::http::schemas::ErrorSchema;
use crate::inbound::http::session::SessionContext;
use crate::inbound::http::state::HttpState;
use crate::inbound::http::validation::{parse_uuid, parse_uuid_list};

/// Request payload for creating a walk session.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateWalkSessionRequestBody {
    pub id: String,
    pub route_id: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub primary_stats: Vec<WalkPrimaryStatBody>,
    pub secondary_stats: Vec<WalkSecondaryStatBody>,
    pub highlighted_poi_ids: Vec<String>,
}

/// Primary walk statistic payload.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WalkPrimaryStatBody {
    pub kind: String,
    pub value: f64,
}

/// Secondary walk statistic payload.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WalkSecondaryStatBody {
    pub kind: String,
    pub value: f64,
    pub unit: Option<String>,
}

/// Response payload for walk session creation.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateWalkSessionResponseBody {
    pub session_id: String,
    pub completion_summary: Option<WalkCompletionSummaryResponseBody>,
}

/// Completion summary payload returned when a walk is complete.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WalkCompletionSummaryResponseBody {
    pub session_id: String,
    pub user_id: String,
    pub route_id: String,
    pub started_at: String,
    pub ended_at: String,
    pub primary_stats: Vec<WalkPrimaryStatBody>,
    pub secondary_stats: Vec<WalkSecondaryStatBody>,
    pub highlighted_poi_ids: Vec<String>,
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

fn parse_optional_timestamp(
    value: Option<String>,
    field: &str,
) -> Result<Option<DateTime<Utc>>, Error> {
    value.map(|raw| parse_timestamp(raw, field)).transpose()
}

#[expect(
    clippy::too_many_arguments,
    reason = "shared helper keeps explicit error metadata parameters at call sites"
)]
fn parse_stat_kind<T>(
    kind: String,
    index: usize,
    field: &str,
    error_message: &str,
    error_code: &str,
    mapper: impl FnOnce(&str) -> Option<T>,
) -> Result<T, Error> {
    mapper(kind.as_str()).ok_or_else(|| {
        Error::invalid_request(error_message).with_details(json!({
            "field": field,
            "index": index,
            "value": kind,
            "code": error_code,
        }))
    })
}

fn parse_stats_collection<TBody, TKind, TDraft>(
    stats: Vec<TBody>,
    parse_kind: impl Fn(String, usize) -> Result<TKind, Error>,
    extract_kind: impl Fn(&TBody) -> String,
    build_draft: impl Fn(TBody, TKind) -> TDraft,
) -> Result<Vec<TDraft>, Error> {
    stats
        .into_iter()
        .enumerate()
        .map(|(index, stat)| {
            let kind = parse_kind(extract_kind(&stat), index)?;
            Ok(build_draft(stat, kind))
        })
        .collect()
}

fn parse_primary_stat_kind(kind: String, index: usize) -> Result<WalkPrimaryStatKind, Error> {
    parse_stat_kind(
        kind,
        index,
        "primaryStats",
        "primaryStats kind must be distance or duration",
        "invalid_primary_stat_kind",
        |kind| match kind {
            "distance" => Some(WalkPrimaryStatKind::Distance),
            "duration" => Some(WalkPrimaryStatKind::Duration),
            _ => None,
        },
    )
}

fn parse_secondary_stat_kind(kind: String, index: usize) -> Result<WalkSecondaryStatKind, Error> {
    parse_stat_kind(
        kind,
        index,
        "secondaryStats",
        "secondaryStats kind must be energy or count",
        "invalid_secondary_stat_kind",
        |kind| match kind {
            "energy" => Some(WalkSecondaryStatKind::Energy),
            "count" => Some(WalkSecondaryStatKind::Count),
            _ => None,
        },
    )
}

fn parse_primary_stats(
    stats: Vec<WalkPrimaryStatBody>,
) -> Result<Vec<WalkPrimaryStatDraft>, Error> {
    parse_stats_collection(
        stats,
        parse_primary_stat_kind,
        |stat| stat.kind.clone(),
        |stat, kind| WalkPrimaryStatDraft {
            kind,
            value: stat.value,
        },
    )
}

fn parse_secondary_stats(
    stats: Vec<WalkSecondaryStatBody>,
) -> Result<Vec<WalkSecondaryStatDraft>, Error> {
    parse_stats_collection(
        stats,
        parse_secondary_stat_kind,
        |stat| stat.kind.clone(),
        |stat, kind| WalkSecondaryStatDraft {
            kind,
            value: stat.value,
            unit: stat.unit,
        },
    )
}

fn parse_walk_session_payload(
    payload: CreateWalkSessionRequestBody,
    user_id: UserId,
) -> Result<WalkSessionPayload, Error> {
    Ok(WalkSessionPayload {
        id: parse_uuid(payload.id, "id")?,
        user_id,
        route_id: parse_uuid(payload.route_id, "routeId")?,
        started_at: parse_timestamp(payload.started_at, "startedAt")?,
        ended_at: parse_optional_timestamp(payload.ended_at, "endedAt")?,
        primary_stats: parse_primary_stats(payload.primary_stats)?,
        secondary_stats: parse_secondary_stats(payload.secondary_stats)?,
        highlighted_poi_ids: parse_uuid_list(payload.highlighted_poi_ids, "highlightedPoiIds")?,
    })
}

impl From<WalkCompletionSummaryPayload> for WalkCompletionSummaryResponseBody {
    fn from(value: WalkCompletionSummaryPayload) -> Self {
        Self {
            session_id: value.session_id.to_string(),
            user_id: value.user_id.to_string(),
            route_id: value.route_id.to_string(),
            started_at: value.started_at.to_rfc3339(),
            ended_at: value.ended_at.to_rfc3339(),
            primary_stats: value
                .primary_stats
                .into_iter()
                .map(|stat| WalkPrimaryStatBody {
                    kind: stat.kind.to_string(),
                    value: stat.value,
                })
                .collect(),
            secondary_stats: value
                .secondary_stats
                .into_iter()
                .map(|stat| WalkSecondaryStatBody {
                    kind: stat.kind.to_string(),
                    value: stat.value,
                    unit: stat.unit,
                })
                .collect(),
            highlighted_poi_ids: value
                .highlighted_poi_ids
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
        }
    }
}

impl From<CreateWalkSessionResponse> for CreateWalkSessionResponseBody {
    fn from(value: CreateWalkSessionResponse) -> Self {
        Self {
            session_id: value.session_id.to_string(),
            completion_summary: value
                .completion_summary
                .map(WalkCompletionSummaryResponseBody::from),
        }
    }
}

/// Record a walk session for the authenticated user.
#[utoipa::path(
    post,
    path = "/api/v1/walk-sessions",
    request_body = CreateWalkSessionRequestBody,
    responses(
        (status = 200, description = "Walk session recorded", body = CreateWalkSessionResponseBody),
        (status = 400, description = "Invalid request", body = ErrorSchema),
        (status = 401, description = "Unauthorised", body = ErrorSchema),
        (status = 503, description = "Service unavailable", body = ErrorSchema)
    ),
    tags = ["walk-sessions"],
    operation_id = "createWalkSession",
    security(("SessionCookie" = []))
)]
#[post("/walk-sessions")]
pub async fn create_walk_session(
    state: web::Data<HttpState>,
    session: SessionContext,
    payload: web::Json<CreateWalkSessionRequestBody>,
) -> ApiResult<web::Json<CreateWalkSessionResponseBody>> {
    let user_id = session.require_user_id()?;
    let session_payload = parse_walk_session_payload(payload.into_inner(), user_id)?;

    let response = state
        .walk_sessions
        .create_session(CreateWalkSessionRequest {
            session: session_payload,
        })
        .await?;

    Ok(web::Json(CreateWalkSessionResponseBody::from(response)))
}

#[cfg(test)]
#[path = "walk_sessions_tests.rs"]
mod tests;
