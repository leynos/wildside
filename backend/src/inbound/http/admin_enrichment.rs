//! Admin endpoint for enrichment provenance reporting.

use actix_web::{HttpResponse, get, web};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::Error;
use crate::domain::enrichment_provenance_error_mapping::map_enrichment_provenance_repository_error;
use crate::domain::ports::{
    EnrichmentProvenanceCursor, EnrichmentProvenanceRecord, EnrichmentProvenanceRepositoryError,
    ListEnrichmentProvenanceRequest,
};
use crate::inbound::http::ApiResult;
use crate::inbound::http::schemas::ErrorSchema;
use crate::inbound::http::session::SessionContext;
use crate::inbound::http::state::HttpState;
use crate::inbound::http::validation::{FieldName, parse_rfc3339_timestamp, parse_uuid};

const DEFAULT_LIMIT: usize = 50;
const MAX_LIMIT: usize = 200;
const BEFORE_CURSOR_SEPARATOR: char = '|';

/// Query parameters for enrichment provenance reporting.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListEnrichmentProvenanceQuery {
    /// Maximum number of rows to return. Defaults to 50, maximum 200.
    pub limit: Option<usize>,
    /// Optional exclusive cursor.
    ///
    /// Accepts either bare `RFC3339` or `RFC3339|UUID` for `(importedAt,id)`.
    /// Bare timestamps are interpreted as `(timestamp, Uuid::max())`.
    pub before: Option<String>,
}

/// Bounds payload for one provenance record.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceBoundsBody {
    pub min_lng: f64,
    pub min_lat: f64,
    pub max_lng: f64,
    pub max_lat: f64,
}

/// JSON record payload for admin enrichment provenance reporting.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnrichmentProvenanceRecordBody {
    pub source_url: String,
    pub imported_at: String,
    pub bounding_box: ProvenanceBoundsBody,
}

/// Response payload for enrichment provenance reporting.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListEnrichmentProvenanceResponseBody {
    pub records: Vec<EnrichmentProvenanceRecordBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_before: Option<String>,
}

impl From<EnrichmentProvenanceRecord> for EnrichmentProvenanceRecordBody {
    fn from(value: EnrichmentProvenanceRecord) -> Self {
        let [min_lng, min_lat, max_lng, max_lat] = value.bounding_box;

        Self {
            source_url: value.source_url,
            imported_at: value.imported_at.to_rfc3339(),
            bounding_box: ProvenanceBoundsBody {
                min_lng,
                min_lat,
                max_lng,
                max_lat,
            },
        }
    }
}

fn parse_limit(limit: Option<usize>) -> Result<usize, Error> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT);
    if (1..=MAX_LIMIT).contains(&limit) {
        return Ok(limit);
    }

    Err(
        Error::invalid_request(format!("limit must be between 1 and {MAX_LIMIT}",)).with_details(
            json!({
                "field": "limit",
                "value": limit,
                "code": "invalid_limit"
            }),
        ),
    )
}

fn map_reporting_error(error: EnrichmentProvenanceRepositoryError) -> Error {
    map_enrichment_provenance_repository_error(
        error,
        "enrichment provenance reporting unavailable",
        "enrichment provenance reporting failed",
    )
}

/// Parse the optional `before` cursor.
///
/// Accepted forms are bare `RFC3339` and `RFC3339|UUID`. Bare timestamps are
/// normalized to `(timestamp, Uuid::max())` so legacy callers still align with
/// `(importedAt,id)` ordering.
fn parse_before_cursor(value: Option<String>) -> Result<Option<(DateTime<Utc>, Uuid)>, Error> {
    let Some(raw) = value else {
        return Ok(None);
    };

    let field = FieldName::new("before");
    if let Some((timestamp_raw, id_raw)) = raw.split_once(BEFORE_CURSOR_SEPARATOR) {
        let timestamp = parse_rfc3339_timestamp(timestamp_raw.to_owned(), field)?;
        let id = parse_uuid(id_raw.to_owned(), field)?;
        return Ok(Some((timestamp, id)));
    }

    let timestamp = parse_rfc3339_timestamp(raw, field)?;
    Ok(Some((timestamp, Uuid::max())))
}

fn encode_before_cursor(cursor: EnrichmentProvenanceCursor) -> String {
    format!(
        "{}{}{}",
        cursor.imported_at.to_rfc3339(),
        BEFORE_CURSOR_SEPARATOR,
        cursor.id
    )
}

/// List persisted enrichment provenance records for admin reporting.
#[utoipa::path(
    get,
    path = "/api/v1/admin/enrichment/provenance",
    params(
        ("limit" = Option<usize>, Query, description = "Number of records to return, default 50, max 200"),
        ("before" = Option<String>, Query, description = "Exclusive cursor RFC3339 or RFC3339|UUID for importedAt/id ordering; bare timestamps map to RFC3339|ffffffff-ffff-ffff-ffff-ffffffffffff")
    ),
    responses(
        (status = 200, description = "Enrichment provenance records", body = ListEnrichmentProvenanceResponseBody),
        (status = 400, description = "Invalid request", body = ErrorSchema),
        (status = 401, description = "Unauthorised", body = ErrorSchema),
        (status = 503, description = "Service unavailable", body = ErrorSchema),
        (status = 500, description = "Internal server error", body = ErrorSchema)
    ),
    tags = ["admin"],
    operation_id = "listEnrichmentProvenance",
    security(("SessionCookie" = []))
)]
#[get("/admin/enrichment/provenance")]
pub async fn list_enrichment_provenance(
    state: web::Data<HttpState>,
    session: SessionContext,
    query: web::Query<ListEnrichmentProvenanceQuery>,
) -> ApiResult<HttpResponse> {
    let _user_id = session.require_admin_user_id()?;
    let query = query.into_inner();
    let limit = parse_limit(query.limit)?;
    let before = parse_before_cursor(query.before)?;

    let response = state
        .enrichment_provenance
        .list_recent(&ListEnrichmentProvenanceRequest::new(limit, before))
        .await
        .map_err(map_reporting_error)?;

    let payload = ListEnrichmentProvenanceResponseBody {
        records: response
            .records
            .into_iter()
            .map(EnrichmentProvenanceRecordBody::from)
            .collect(),
        next_before: response.next_before.map(encode_before_cursor),
    };

    Ok(HttpResponse::Ok().json(payload))
}
