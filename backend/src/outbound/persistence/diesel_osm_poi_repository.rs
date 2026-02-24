//! PostgreSQL-backed OSM POI ingestion adapter.

use diesel::sql_query;
use diesel::sql_types::{BigInt, Double, Jsonb, Text};
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
VALUES ($1, $2, point($3, $4), $5, NULL, 0)
ON CONFLICT (element_type, id)
DO UPDATE SET
    location = EXCLUDED.location,
    osm_tags = EXCLUDED.osm_tags,
    narrative = EXCLUDED.narrative,
    popularity_score = EXCLUDED.popularity_score
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

        let serialized_records = records
            .iter()
            .map(|record| {
                let tags = serde_json::to_value(&record.tags).map_err(|err| {
                    OsmPoiRepositoryError::query(format!(
                        "failed to serialize OSM tags for {}:{}: {err}",
                        record.element_type, record.element_id
                    ))
                })?;
                Ok((
                    record.element_type.clone(),
                    record.element_id,
                    record.longitude,
                    record.latitude,
                    tags,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut conn = self.pool.get().await.map_err(map_pool_error)?;
        for (element_type, element_id, longitude, latitude, tags) in &serialized_records {
            sql_query(UPSERT_SQL)
                .bind::<Text, _>(element_type)
                .bind::<BigInt, _>(element_id)
                .bind::<Double, _>(longitude)
                .bind::<Double, _>(latitude)
                .bind::<Jsonb, _>(tags)
                .execute(&mut conn)
                .await
                .map_err(map_diesel_error)?;
        }

        Ok(())
    }
}
