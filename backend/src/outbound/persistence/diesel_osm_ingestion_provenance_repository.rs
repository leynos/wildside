//! PostgreSQL-backed ingestion provenance adapter.

use chrono::{DateTime, Utc};
use diesel::OptionalExtension;
use diesel::prelude::*;
use diesel::result::DatabaseErrorKind;
use diesel::sql_query;
use diesel::sql_types::{Array, BigInt, Double, Jsonb, Text};
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::domain::ports::{
    OsmIngestionProvenanceRecord, OsmIngestionProvenanceRepository,
    OsmIngestionProvenanceRepositoryError, OsmPoiIngestionRecord,
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
    raw_poi_count: i64,
    filtered_poi_count: i64,
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
    raw_poi_count: i64,
    filtered_poi_count: i64,
}

struct PoiUpsertBatch {
    element_types: Vec<String>,
    element_ids: Vec<i64>,
    longitudes: Vec<f64>,
    latitudes: Vec<f64>,
    tags: Vec<serde_json::Value>,
}

const UPSERT_POIS_SQL: &str = r#"
INSERT INTO pois (element_type, id, location, osm_tags, narrative, popularity_score)
SELECT
    source.element_type,
    source.id,
    point(source.longitude, source.latitude),
    source.osm_tags,
    NULL,
    0
FROM unnest(
    $1::text[],
    $2::bigint[],
    $3::double precision[],
    $4::double precision[],
    $5::jsonb[]
) AS source(element_type, id, longitude, latitude, osm_tags)
ON CONFLICT (element_type, id)
DO UPDATE SET
    location = EXCLUDED.location,
    osm_tags = EXCLUDED.osm_tags
"#;

fn map_pool_error(error: PoolError) -> OsmIngestionProvenanceRepositoryError {
    OsmIngestionProvenanceRepositoryError::connection(map_pool_error_message(error))
}

fn map_diesel_error(error: diesel::result::Error) -> OsmIngestionProvenanceRepositoryError {
    OsmIngestionProvenanceRepositoryError::query(map_diesel_error_message(
        error,
        "osm ingestion provenance operation",
    ))
}

fn map_atomic_persist_error(error: diesel::result::Error) -> OsmIngestionProvenanceRepositoryError {
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
        // `id` and `created_at` remain persistence-local metadata and are
        // intentionally excluded from the domain provenance contract.
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
    let raw_poi_count = i64::try_from(record.raw_poi_count).map_err(|_| {
        OsmIngestionProvenanceRepositoryError::query(
            "raw_poi_count exceeds supported i64 range for persistence",
        )
    })?;
    let filtered_poi_count = i64::try_from(record.filtered_poi_count).map_err(|_| {
        OsmIngestionProvenanceRepositoryError::query(
            "filtered_poi_count exceeds supported i64 range for persistence",
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

fn to_poi_upsert_batch(
    records: &[OsmPoiIngestionRecord],
) -> Result<Option<PoiUpsertBatch>, OsmIngestionProvenanceRepositoryError> {
    if records.is_empty() {
        return Ok(None);
    }

    let element_types = records
        .iter()
        .map(|record| record.element_type.clone())
        .collect::<Vec<_>>();
    let element_ids = records
        .iter()
        .map(|record| record.element_id)
        .collect::<Vec<_>>();
    let longitudes = records
        .iter()
        .map(|record| record.longitude)
        .collect::<Vec<_>>();
    let latitudes = records
        .iter()
        .map(|record| record.latitude)
        .collect::<Vec<_>>();
    let tags = records
        .iter()
        .map(|record| {
            serde_json::to_value(&record.tags).map_err(|error| {
                OsmIngestionProvenanceRepositoryError::query(format!(
                    "failed to serialize OSM tags for {}:{}: {error}",
                    record.element_type, record.element_id
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Some(PoiUpsertBatch {
        element_types,
        element_ids,
        longitudes,
        latitudes,
        tags,
    }))
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

    async fn persist_ingestion(
        &self,
        provenance: &OsmIngestionProvenanceRecord,
        poi_records: &[OsmPoiIngestionRecord],
    ) -> Result<(), OsmIngestionProvenanceRepositoryError> {
        use diesel_async::AsyncConnection as _;
        use diesel_async::scoped_futures::ScopedFutureExt as _;

        let provenance_row = to_insert_row(provenance)?;
        let poi_batch = to_poi_upsert_batch(poi_records)?;
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        // Keep provenance insert and POI upsert in one transaction so a rerun
        // key conflict or write error cannot leave partial ingest state.
        conn.transaction(|conn| {
            async move {
                diesel::insert_into(osm_ingestion_provenance::table)
                    .values(&provenance_row)
                    .execute(conn)
                    .await?;

                if let Some(batch) = &poi_batch {
                    sql_query(UPSERT_POIS_SQL)
                        .bind::<Array<Text>, _>(&batch.element_types)
                        .bind::<Array<BigInt>, _>(&batch.element_ids)
                        .bind::<Array<Double>, _>(&batch.longitudes)
                        .bind::<Array<Double>, _>(&batch.latitudes)
                        .bind::<Array<Jsonb>, _>(&batch.tags)
                        .execute(conn)
                        .await?;
                }

                Ok(())
            }
            .scope_boxed()
        })
        .await
        .map_err(map_atomic_persist_error)?;

        Ok(())
    }
}
