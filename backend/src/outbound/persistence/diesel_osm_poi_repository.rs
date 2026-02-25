//! PostgreSQL-backed OSM POI ingestion adapter.
//!
//! This adapter batches geofenced POIs into one UPSERT statement so ingest runs
//! avoid per-row round-trips. Only OSM-owned columns (`location`, `osm_tags`)
//! are refreshed on conflicts, preserving user-curated `narrative` text and
//! downstream-computed `popularity_score`.

use diesel::sql_query;
use diesel::sql_types::{Array, BigInt, Double, Jsonb, Text};
use diesel_async::RunQueryDsl;

use crate::domain::ports::{OsmPoiIngestionRecord, OsmPoiRepository, OsmPoiRepositoryError};

use super::diesel_helpers::{map_diesel_error_message, map_pool_error_message};
use super::pool::{DbPool, PoolError};

/// Diesel-backed implementation of the OSM POI ingestion port.
#[derive(Clone)]
pub struct DieselOsmPoiRepository {
    pool: DbPool,
}

impl DieselOsmPoiRepository {
    /// Create a new repository with the given connection pool.
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

const UPSERT_SQL: &str = r#"
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

fn map_pool_error(error: PoolError) -> OsmPoiRepositoryError {
    OsmPoiRepositoryError::connection(map_pool_error_message(error))
}

fn map_diesel_error(error: diesel::result::Error) -> OsmPoiRepositoryError {
    OsmPoiRepositoryError::query(map_diesel_error_message(error, "osm poi ingestion upsert"))
}

#[async_trait::async_trait]
impl OsmPoiRepository for DieselOsmPoiRepository {
    async fn upsert_pois(
        &self,
        records: &[OsmPoiIngestionRecord],
    ) -> Result<(), OsmPoiRepositoryError> {
        if records.is_empty() {
            return Ok(());
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
                    OsmPoiRepositoryError::query(format!(
                        "failed to serialize OSM tags for {}:{}: {error}",
                        record.element_type, record.element_id
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut conn = self.pool.get().await.map_err(map_pool_error)?;
        sql_query(UPSERT_SQL)
            .bind::<Array<Text>, _>(&element_types)
            .bind::<Array<BigInt>, _>(&element_ids)
            .bind::<Array<Double>, _>(&longitudes)
            .bind::<Array<Double>, _>(&latitudes)
            .bind::<Array<Jsonb>, _>(&tags)
            .execute(&mut conn)
            .await
            .map_err(map_diesel_error)?;

        Ok(())
    }
}
