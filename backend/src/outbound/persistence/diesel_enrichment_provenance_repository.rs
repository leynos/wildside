//! PostgreSQL-backed adapter for Overpass enrichment provenance records.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::pooled_connection::bb8::PooledConnection;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use uuid::Uuid;

use crate::domain::ports::{
    EnrichmentProvenanceCursor, EnrichmentProvenanceRecord, EnrichmentProvenanceRepository,
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

    fn build_base_query<'a>(&self, request: &'a ListEnrichmentProvenanceRequest) -> BoxedQuery<'a> {
        let mut query = overpass_enrichment_provenance::table
            .select(EnrichmentProvenanceRow::as_select())
            .order((
                overpass_enrichment_provenance::imported_at.desc(),
                overpass_enrichment_provenance::id.desc(),
            ))
            .into_boxed();

        if let Some(before) = request.before {
            query = query.filter(
                overpass_enrichment_provenance::imported_at
                    .lt(before.imported_at)
                    .or(overpass_enrichment_provenance::imported_at
                        .eq(before.imported_at)
                        .and(overpass_enrichment_provenance::id.lt(before.id))),
            );
        }

        query
    }

    async fn load_rows_with_limit(
        &self,
        conn: &mut PooledPgConnection<'_>,
        query: BoxedQuery<'_>,
        limit: i64,
    ) -> Result<Vec<EnrichmentProvenanceRow>, EnrichmentProvenanceRepositoryError> {
        query
            .limit(limit)
            .load::<EnrichmentProvenanceRow>(conn)
            .await
            .map_err(map_diesel_error)
    }

    async fn collect_boundary_rows(
        &self,
        conn: &mut PooledPgConnection<'_>,
        request: BoundaryRowsRequest,
    ) -> Result<Vec<EnrichmentProvenanceRow>, EnrichmentProvenanceRepositoryError> {
        let BoundaryRowsRequest {
            boundary_imported_at,
            before,
            remaining,
        } = request;
        let mut query = overpass_enrichment_provenance::table
            .select(EnrichmentProvenanceRow::as_select())
            .filter(overpass_enrichment_provenance::imported_at.eq(boundary_imported_at))
            .order(overpass_enrichment_provenance::id.desc())
            .into_boxed();

        if let Some(before) = before
            && before.imported_at == boundary_imported_at
        {
            query = query.filter(overpass_enrichment_provenance::id.lt(before.id));
        }

        query
            .limit((remaining as i64) + 1)
            .load::<EnrichmentProvenanceRow>(conn)
            .await
            .map_err(map_diesel_error)
    }

    async fn derive_next_before(
        &self,
        conn: &mut PooledPgConnection<'_>,
        cursor: EnrichmentProvenanceCursor,
    ) -> Result<Option<EnrichmentProvenanceCursor>, EnrichmentProvenanceRepositoryError> {
        let has_older_rows = !overpass_enrichment_provenance::table
            .select(overpass_enrichment_provenance::id)
            .filter(
                overpass_enrichment_provenance::imported_at
                    .lt(cursor.imported_at)
                    .or(overpass_enrichment_provenance::imported_at
                        .eq(cursor.imported_at)
                        .and(overpass_enrichment_provenance::id.lt(cursor.id))),
            )
            .limit(1)
            .load::<Uuid>(conn)
            .await
            .map_err(map_diesel_error)?
            .is_empty();

        Ok(has_older_rows.then_some(cursor))
    }

    fn split_boundary_page_rows(
        rows: Vec<EnrichmentProvenanceRow>,
        boundary_rows: Vec<EnrichmentProvenanceRow>,
        limit: usize,
        boundary_imported_at: DateTime<Utc>,
    ) -> (
        Vec<EnrichmentProvenanceRow>,
        Option<EnrichmentProvenanceCursor>,
    ) {
        let mut page_rows = rows
            .into_iter()
            .take_while(|row| row.imported_at > boundary_imported_at)
            .collect::<Vec<_>>();

        let remaining = limit.saturating_sub(page_rows.len());
        page_rows.extend(boundary_rows.into_iter().take(remaining));

        let next_before = page_rows
            .last()
            .map(|row| EnrichmentProvenanceCursor::new(row.imported_at, row.id));

        (page_rows, next_before)
    }

    /// Handle split-boundary pagination when the page break falls within rows
    /// that share the same `imported_at` timestamp.
    async fn handle_split_boundary(
        &self,
        conn: &mut DbConn<'_>,
        context: SplitBoundaryContext<'_>,
    ) -> Result<ListEnrichmentProvenanceResponse, EnrichmentProvenanceRepositoryError> {
        let SplitBoundaryContext {
            rows,
            request,
            boundary_imported_at,
        } = context;
        let newer_rows_count = rows
            .iter()
            .take_while(|row| row.imported_at > boundary_imported_at)
            .count();
        let remaining = request.limit.saturating_sub(newer_rows_count);
        let boundary_rows = self
            .collect_boundary_rows(
                conn,
                BoundaryRowsRequest {
                    boundary_imported_at,
                    before: request.before,
                    remaining,
                },
            )
            .await?;
        let boundary_has_overflow = boundary_rows.len() > remaining;
        let (page_rows, cursor) = Self::split_boundary_page_rows(
            rows,
            boundary_rows,
            request.limit,
            boundary_imported_at,
        );
        let records = page_rows
            .into_iter()
            .map(EnrichmentProvenanceRecord::from)
            .collect::<Vec<_>>();
        let next_before = if let Some(cursor) = cursor {
            if boundary_has_overflow {
                Some(cursor)
            } else {
                self.derive_next_before(conn, cursor).await?
            }
        } else {
            None
        };

        Ok(ListEnrichmentProvenanceResponse {
            records,
            next_before,
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct BoundaryRowsRequest {
    boundary_imported_at: DateTime<Utc>,
    before: Option<EnrichmentProvenanceCursor>,
    remaining: usize,
}

struct SplitBoundaryContext<'a> {
    rows: Vec<EnrichmentProvenanceRow>,
    request: &'a ListEnrichmentProvenanceRequest,
    boundary_imported_at: DateTime<Utc>,
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

type BoxedQuery<'a> = diesel::dsl::IntoBoxed<
    'a,
    diesel::dsl::Select<
        overpass_enrichment_provenance::table,
        diesel::dsl::AsSelect<EnrichmentProvenanceRow, diesel::pg::Pg>,
    >,
    diesel::pg::Pg,
>;

type PooledPgConnection<'a> = PooledConnection<'a, AsyncPgConnection>;
type DbConn<'a> = PooledPgConnection<'a>;

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
        let query = self.build_base_query(request);
        let mut rows = self
            .load_rows_with_limit(&mut conn, query, limit_i64)
            .await?;

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
            let next_before = rows
                .last()
                .map(|row| EnrichmentProvenanceCursor::new(row.imported_at, row.id));
            let records = rows
                .into_iter()
                .map(EnrichmentProvenanceRecord::from)
                .collect::<Vec<_>>();
            return Ok(ListEnrichmentProvenanceResponse {
                records,
                next_before,
            });
        }

        self.handle_split_boundary(
            &mut conn,
            SplitBoundaryContext {
                rows,
                request,
                boundary_imported_at,
            },
        )
        .await
    }
}

#[cfg(test)]
mod tests;
