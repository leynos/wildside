//! PostgreSQL-backed adapter for Overpass enrichment provenance records.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::domain::ports::{
    EnrichmentProvenanceRecord, EnrichmentProvenanceRepository,
    EnrichmentProvenanceRepositoryError, ListEnrichmentProvenanceRequest,
    ListEnrichmentProvenanceResponse,
};

use super::diesel_helpers::{map_diesel_error_message, map_pool_error_message};
use super::pool::{DbPool, PoolError};
use super::schema::overpass_enrichment_provenance;

/// Diesel-backed implementation of [`EnrichmentProvenanceRepository`].
#[derive(Clone)]
pub struct DieselEnrichmentProvenanceRepository {
    pool: DbPool,
}

impl DieselEnrichmentProvenanceRepository {
    /// Create a repository backed by `pool`.
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = overpass_enrichment_provenance)]
struct EnrichmentProvenanceRow {
    id: Uuid,
    source_url: String,
    imported_at: DateTime<Utc>,
    bounds_min_lng: f64,
    bounds_min_lat: f64,
    bounds_max_lng: f64,
    bounds_max_lat: f64,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = overpass_enrichment_provenance)]
struct NewEnrichmentProvenanceRow<'a> {
    source_url: &'a str,
    imported_at: DateTime<Utc>,
    bounds_min_lng: f64,
    bounds_min_lat: f64,
    bounds_max_lng: f64,
    bounds_max_lat: f64,
}

fn map_pool_error(error: PoolError) -> EnrichmentProvenanceRepositoryError {
    EnrichmentProvenanceRepositoryError::connection(map_pool_error_message(error))
}

fn map_diesel_error(error: diesel::result::Error) -> EnrichmentProvenanceRepositoryError {
    EnrichmentProvenanceRepositoryError::query(map_diesel_error_message(
        error,
        "enrichment provenance operation",
    ))
}

impl From<EnrichmentProvenanceRow> for EnrichmentProvenanceRecord {
    fn from(row: EnrichmentProvenanceRow) -> Self {
        // `id` and `created_at` stay persistence-local metadata.
        let _ = row.id;
        let _ = row.created_at;

        Self {
            source_url: row.source_url,
            imported_at: row.imported_at,
            bounding_box: [
                row.bounds_min_lng,
                row.bounds_min_lat,
                row.bounds_max_lng,
                row.bounds_max_lat,
            ],
        }
    }
}

fn to_insert_row(record: &EnrichmentProvenanceRecord) -> NewEnrichmentProvenanceRow<'_> {
    let [
        bounds_min_lng,
        bounds_min_lat,
        bounds_max_lng,
        bounds_max_lat,
    ] = record.bounding_box;
    NewEnrichmentProvenanceRow {
        source_url: record.source_url.as_str(),
        imported_at: record.imported_at,
        bounds_min_lng,
        bounds_min_lat,
        bounds_max_lng,
        bounds_max_lat,
    }
}

#[async_trait]
impl EnrichmentProvenanceRepository for DieselEnrichmentProvenanceRepository {
    async fn persist(
        &self,
        record: &EnrichmentProvenanceRecord,
    ) -> Result<(), EnrichmentProvenanceRepositoryError> {
        let row = to_insert_row(record);
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        diesel::insert_into(overpass_enrichment_provenance::table)
            .values(&row)
            .execute(&mut conn)
            .await
            .map(|_| ())
            .map_err(map_diesel_error)
    }

    async fn list_recent(
        &self,
        request: &ListEnrichmentProvenanceRequest,
    ) -> Result<ListEnrichmentProvenanceResponse, EnrichmentProvenanceRepositoryError> {
        if request.limit == 0 {
            return Ok(ListEnrichmentProvenanceResponse {
                records: Vec::new(),
                next_before: None,
            });
        }

        let limit_i64 = i64::try_from(request.limit.saturating_add(1)).map_err(|_| {
            EnrichmentProvenanceRepositoryError::query("requested limit exceeds i64 range")
        })?;

        let mut conn = self.pool.get().await.map_err(map_pool_error)?;
        let mut query = overpass_enrichment_provenance::table
            .select(EnrichmentProvenanceRow::as_select())
            .order((
                overpass_enrichment_provenance::imported_at.desc(),
                overpass_enrichment_provenance::id.desc(),
            ))
            .into_boxed();

        if let Some(before) = request.before {
            query = query.filter(overpass_enrichment_provenance::imported_at.lt(before));
        }

        let mut rows = query
            .limit(limit_i64)
            .load::<EnrichmentProvenanceRow>(&mut conn)
            .await
            .map_err(map_diesel_error)?;

        let has_next = rows.len() > request.limit;
        if !has_next {
            let records = rows
                .into_iter()
                .map(EnrichmentProvenanceRecord::from)
                .collect::<Vec<_>>();
            return Ok(ListEnrichmentProvenanceResponse {
                records,
                next_before: None,
            });
        }

        let boundary_imported_at = rows[request.limit - 1].imported_at;
        let boundary_is_split = rows[request.limit].imported_at == boundary_imported_at;

        if !boundary_is_split {
            rows.truncate(request.limit);
            let records = rows
                .into_iter()
                .map(EnrichmentProvenanceRecord::from)
                .collect::<Vec<_>>();
            let next_before = records.last().map(|record| record.imported_at);
            return Ok(ListEnrichmentProvenanceResponse {
                records,
                next_before,
            });
        }

        // Avoid splitting a page inside a shared imported_at bucket: this keeps
        // pagination lossless even when multiple rows share the cursor timestamp.
        let mut records = rows
            .into_iter()
            .take_while(|row| row.imported_at > boundary_imported_at)
            .map(EnrichmentProvenanceRecord::from)
            .collect::<Vec<_>>();

        let mut boundary_rows_query = overpass_enrichment_provenance::table
            .select(EnrichmentProvenanceRow::as_select())
            .filter(overpass_enrichment_provenance::imported_at.eq(boundary_imported_at))
            .order(overpass_enrichment_provenance::id.desc())
            .into_boxed();

        if let Some(before) = request.before {
            boundary_rows_query =
                boundary_rows_query.filter(overpass_enrichment_provenance::imported_at.lt(before));
        }

        let boundary_records = boundary_rows_query
            .load::<EnrichmentProvenanceRow>(&mut conn)
            .await
            .map_err(map_diesel_error)?
            .into_iter()
            .map(EnrichmentProvenanceRecord::from);
        records.extend(boundary_records);

        let mut has_older_rows_query = overpass_enrichment_provenance::table
            .select(overpass_enrichment_provenance::id)
            .filter(overpass_enrichment_provenance::imported_at.lt(boundary_imported_at))
            .into_boxed();
        if let Some(before) = request.before {
            has_older_rows_query =
                has_older_rows_query.filter(overpass_enrichment_provenance::imported_at.lt(before));
        }
        let has_older_rows = !has_older_rows_query
            .limit(1)
            .load::<Uuid>(&mut conn)
            .await
            .map_err(map_diesel_error)?
            .is_empty();

        let next_before = if has_older_rows {
            Some(boundary_imported_at)
        } else {
            None
        };

        Ok(ListEnrichmentProvenanceResponse {
            records,
            next_before,
        })
    }
}
