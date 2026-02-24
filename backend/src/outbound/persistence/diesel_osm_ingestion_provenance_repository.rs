//! PostgreSQL-backed ingestion provenance adapter.

use chrono::{DateTime, Utc};
use diesel::OptionalExtension;
use diesel::prelude::*;
use diesel::result::DatabaseErrorKind;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::domain::ports::{
    OsmIngestionProvenanceRecord, OsmIngestionProvenanceRepository,
    OsmIngestionProvenanceRepositoryError,
};

use super::diesel_helpers::{map_diesel_error_message, map_pool_error_message};
use super::pool::{DbPool, PoolError};
use super::schema::osm_ingestion_provenance;

/// Diesel-backed implementation of the ingestion provenance port.
#[derive(Clone)]
pub struct DieselOsmIngestionProvenanceRepository {
    pool: DbPool,
}

impl DieselOsmIngestionProvenanceRepository {
    /// Create a new repository with the given connection pool.
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = osm_ingestion_provenance)]
struct OsmIngestionProvenanceRow {
    id: Uuid,
    geofence_id: String,
    source_url: String,
    input_digest: String,
    imported_at: DateTime<Utc>,
    bounds_min_lng: f64,
    bounds_min_lat: f64,
    bounds_max_lng: f64,
    bounds_max_lat: f64,
    raw_poi_count: i32,
    filtered_poi_count: i32,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = osm_ingestion_provenance)]
struct NewOsmIngestionProvenanceRow<'a> {
    geofence_id: &'a str,
    source_url: &'a str,
    input_digest: &'a str,
    imported_at: DateTime<Utc>,
    bounds_min_lng: f64,
    bounds_min_lat: f64,
    bounds_max_lng: f64,
    bounds_max_lat: f64,
    raw_poi_count: i32,
    filtered_poi_count: i32,
}

fn map_pool_error(error: PoolError) -> OsmIngestionProvenanceRepositoryError {
    OsmIngestionProvenanceRepositoryError::connection(map_pool_error_message(error))
}

fn map_diesel_error(error: diesel::result::Error) -> OsmIngestionProvenanceRepositoryError {
    OsmIngestionProvenanceRepositoryError::query(map_diesel_error_message(
        error,
        "osm ingestion provenance operation",
    ))
}

fn map_insert_error(error: diesel::result::Error) -> OsmIngestionProvenanceRepositoryError {
    match &error {
        diesel::result::Error::DatabaseError(DatabaseErrorKind::UniqueViolation, info) => {
            OsmIngestionProvenanceRepositoryError::conflict(info.message())
        }
        _ => map_diesel_error(error),
    }
}

impl TryFrom<OsmIngestionProvenanceRow> for OsmIngestionProvenanceRecord {
    type Error = OsmIngestionProvenanceRepositoryError;

    fn try_from(row: OsmIngestionProvenanceRow) -> Result<Self, Self::Error> {
        let _ = row.id;
        let _ = row.created_at;
        let raw_poi_count = u64::try_from(row.raw_poi_count).map_err(|_| {
            OsmIngestionProvenanceRepositoryError::query(
                "raw_poi_count is negative in osm_ingestion_provenance row",
            )
        })?;
        let filtered_poi_count = u64::try_from(row.filtered_poi_count).map_err(|_| {
            OsmIngestionProvenanceRepositoryError::query(
                "filtered_poi_count is negative in osm_ingestion_provenance row",
            )
        })?;

        Ok(Self {
            geofence_id: row.geofence_id,
            source_url: row.source_url,
            input_digest: row.input_digest,
            imported_at: row.imported_at,
            geofence_bounds: [
                row.bounds_min_lng,
                row.bounds_min_lat,
                row.bounds_max_lng,
                row.bounds_max_lat,
            ],
            raw_poi_count,
            filtered_poi_count,
        })
    }
}

fn to_insert_row(
    record: &OsmIngestionProvenanceRecord,
) -> Result<NewOsmIngestionProvenanceRow<'_>, OsmIngestionProvenanceRepositoryError> {
    let [
        bounds_min_lng,
        bounds_min_lat,
        bounds_max_lng,
        bounds_max_lat,
    ] = record.geofence_bounds;
    let raw_poi_count = i32::try_from(record.raw_poi_count).map_err(|_| {
        OsmIngestionProvenanceRepositoryError::query(
            "raw_poi_count exceeds supported i32 range for persistence",
        )
    })?;
    let filtered_poi_count = i32::try_from(record.filtered_poi_count).map_err(|_| {
        OsmIngestionProvenanceRepositoryError::query(
            "filtered_poi_count exceeds supported i32 range for persistence",
        )
    })?;
    Ok(NewOsmIngestionProvenanceRow {
        geofence_id: record.geofence_id.as_str(),
        source_url: record.source_url.as_str(),
        input_digest: record.input_digest.as_str(),
        imported_at: record.imported_at,
        bounds_min_lng,
        bounds_min_lat,
        bounds_max_lng,
        bounds_max_lat,
        raw_poi_count,
        filtered_poi_count,
    })
}

#[async_trait::async_trait]
impl OsmIngestionProvenanceRepository for DieselOsmIngestionProvenanceRepository {
    async fn find_by_rerun_key(
        &self,
        geofence_id: &str,
        input_digest: &str,
    ) -> Result<Option<OsmIngestionProvenanceRecord>, OsmIngestionProvenanceRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;
        let row = osm_ingestion_provenance::table
            .filter(osm_ingestion_provenance::geofence_id.eq(geofence_id))
            .filter(osm_ingestion_provenance::input_digest.eq(input_digest))
            .select(OsmIngestionProvenanceRow::as_select())
            .first(&mut conn)
            .await
            .optional()
            .map_err(map_diesel_error)?;
        row.map(TryInto::try_into).transpose()
    }

    async fn insert(
        &self,
        record: &OsmIngestionProvenanceRecord,
    ) -> Result<(), OsmIngestionProvenanceRepositoryError> {
        let new_row = to_insert_row(record)?;
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;
        diesel::insert_into(osm_ingestion_provenance::table)
            .values(&new_row)
            .execute(&mut conn)
            .await
            .map_err(map_insert_error)?;
        Ok(())
    }
}
