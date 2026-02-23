//! Walk session HTTP handlers.
//!
//! ```text
//! POST /api/v1/walk-sessions
//! ```

use std::str::FromStr;

use actix_web::{post, web};
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
use crate::inbound::http::validation::{
    FieldName, parse_optional_rfc3339_timestamp, parse_rfc3339_timestamp, parse_uuid,
    parse_uuid_list,
};

/// Request payload for creating a walk session.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateWalkSessionRequestBody {
    #[schema(format = "uuid")]
    pub id: String,
    #[schema(format = "uuid")]
    pub route_id: String,
    #[schema(format = "date-time")]
    pub started_at: String,
    #[schema(format = "date-time")]
    pub ended_at: Option<String>,
    pub primary_stats: Vec<WalkPrimaryStatBody>,
    pub secondary_stats: Vec<WalkSecondaryStatBody>,
    #[schema(value_type = Vec<uuid::Uuid>)]
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
    #[schema(format = "uuid")]
    pub session_id: String,
    pub completion_summary: Option<WalkCompletionSummaryResponseBody>,
}

/// Completion summary payload returned when a walk is complete.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WalkCompletionSummaryResponseBody {
    #[schema(format = "uuid")]
    pub session_id: String,
    #[schema(format = "uuid")]
    pub user_id: String,
    #[schema(format = "uuid")]
    pub route_id: String,
    #[schema(format = "date-time")]
    pub started_at: String,
    #[schema(format = "date-time")]
    pub ended_at: String,
    pub primary_stats: Vec<WalkPrimaryStatBody>,
    pub secondary_stats: Vec<WalkSecondaryStatBody>,
    #[schema(value_type = Vec<uuid::Uuid>)]
    pub highlighted_poi_ids: Vec<String>,
}

fn parse_primary_stats(
    stats: Vec<WalkPrimaryStatBody>,
) -> Result<Vec<WalkPrimaryStatDraft>, Error> {
    let mut parsed = Vec::with_capacity(stats.len());
    for (index, stat) in stats.into_iter().enumerate() {
        let kind = WalkPrimaryStatKind::from_str(stat.kind.as_str()).map_err(|_| {
            Error::invalid_request("primaryStats kind must be distance or duration").with_details(
                json!({
                    "field": "primaryStats",
                    "index": index,
                    "value": stat.kind,
                    "code": "invalid_primary_stat_kind",
                }),
            )
        })?;
        parsed.push(WalkPrimaryStatDraft {
            kind,
            value: stat.value,
        });
    }
    Ok(parsed)
}

fn parse_secondary_stats(
    stats: Vec<WalkSecondaryStatBody>,
) -> Result<Vec<WalkSecondaryStatDraft>, Error> {
    let mut parsed = Vec::with_capacity(stats.len());
    for (index, stat) in stats.into_iter().enumerate() {
        let kind = WalkSecondaryStatKind::from_str(stat.kind.as_str()).map_err(|_| {
            Error::invalid_request("secondaryStats kind must be energy or count").with_details(
                json!({
                    "field": "secondaryStats",
                    "index": index,
                    "value": stat.kind,
                    "code": "invalid_secondary_stat_kind",
                }),
            )
        })?;
        parsed.push(WalkSecondaryStatDraft {
            kind,
            value: stat.value,
            unit: stat.unit,
        });
    }
    Ok(parsed)
}

fn parse_walk_session_payload(
    payload: CreateWalkSessionRequestBody,
    user_id: UserId,
) -> Result<WalkSessionPayload, Error> {
    Ok(WalkSessionPayload {
        id: parse_uuid(payload.id, FieldName::new("id"))?,
        user_id,
        route_id: parse_uuid(payload.route_id, FieldName::new("routeId"))?,
        started_at: parse_rfc3339_timestamp(payload.started_at, FieldName::new("startedAt"))?,
        ended_at: parse_optional_rfc3339_timestamp(payload.ended_at, FieldName::new("endedAt"))?,
        primary_stats: parse_primary_stats(payload.primary_stats)?,
        secondary_stats: parse_secondary_stats(payload.secondary_stats)?,
        highlighted_poi_ids: parse_uuid_list(
            payload.highlighted_poi_ids,
            FieldName::new("highlightedPoiIds"),
        )?,
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
///
/// # Examples
/// ```no_run
/// use actix_web::web;
/// use backend::inbound::http::walk_sessions::{
///     CreateWalkSessionRequestBody, WalkPrimaryStatBody, create_walk_session,
/// };
/// use backend::inbound::http::{ApiResult, state::HttpState};
/// use backend::inbound::http::session::SessionContext;
///
/// async fn call_handler(
///     state: web::Data<HttpState>,
///     session: SessionContext,
/// ) -> ApiResult<web::Json<backend::inbound::http::walk_sessions::CreateWalkSessionResponseBody>>
/// {
///     let payload = web::Json(CreateWalkSessionRequestBody {
///         id: "00000000-0000-0000-0000-000000000501".to_owned(),
///         route_id: "00000000-0000-0000-0000-000000000202".to_owned(),
///         started_at: "2026-02-01T11:00:00Z".to_owned(),
///         ended_at: Some("2026-02-01T11:40:00Z".to_owned()),
///         primary_stats: vec![WalkPrimaryStatBody {
///             kind: "distance".to_owned(),
///             value: 1234.0,
///         }],
///         secondary_stats: vec![],
///         highlighted_poi_ids: vec![],
///     });
///
///     let response = create_walk_session(state, session, payload).await?;
///     Ok(response)
/// }
/// ```
#[utoipa::path(
    post,
    path = "/api/v1/walk-sessions",
    request_body = CreateWalkSessionRequestBody,
    responses(
        (status = 200, description = "Walk session recorded", body = CreateWalkSessionResponseBody),
        (status = 400, description = "Invalid request", body = ErrorSchema),
        (status = 401, description = "Unauthorized", body = ErrorSchema),
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
