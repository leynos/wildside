//! PostgreSQL-backed descriptor ingestion adapter.

use async_trait::async_trait;
use diesel::prelude::*;
use diesel::upsert::excluded;
use diesel_async::RunQueryDsl;
use tracing::debug;

use crate::domain::ports::{
    BadgeIngestion, DescriptorIngestionRepository, DescriptorIngestionRepositoryError,
    InterestThemeIngestion, SafetyPresetIngestion, SafetyToggleIngestion, TagIngestion,
};

use super::models::{
    NewBadgeRow, NewInterestThemeRow, NewSafetyPresetRow, NewSafetyToggleRow, NewTagRow,
};
use super::pool::{DbPool, PoolError};
use super::schema::{badges, interest_themes, safety_presets, safety_toggles, tags};

/// Diesel-backed implementation of the descriptor ingestion port.
#[derive(Clone)]
pub struct DieselDescriptorIngestionRepository {
    pool: DbPool,
}

impl DieselDescriptorIngestionRepository {
    /// Create a new repository with the given connection pool.
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

fn map_pool_error(error: PoolError) -> DescriptorIngestionRepositoryError {
    match error {
        PoolError::Checkout { message } | PoolError::Build { message } => {
            DescriptorIngestionRepositoryError::connection(message)
        }
    }
}

fn map_diesel_error(error: diesel::result::Error) -> DescriptorIngestionRepositoryError {
    let error_message = error.to_string();
    debug!(%error_message, "descriptor diesel operation failed");
    DescriptorIngestionRepositoryError::query(error_message)
}

#[async_trait]
impl DescriptorIngestionRepository for DieselDescriptorIngestionRepository {
    async fn upsert_tags(
        &self,
        records: &[TagIngestion],
    ) -> Result<(), DescriptorIngestionRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;
        for record in records {
            let row = NewTagRow {
                id: record.id,
                slug: record.slug.as_str(),
                icon_key: record.icon_key.as_str(),
                localizations: &record.localizations,
            };
            diesel::insert_into(tags::table)
                .values(&row)
                .on_conflict(tags::id)
                .do_update()
                .set((
                    tags::slug.eq(excluded(tags::slug)),
                    tags::icon_key.eq(excluded(tags::icon_key)),
                    tags::localizations.eq(excluded(tags::localizations)),
                ))
                .execute(&mut conn)
                .await
                .map_err(map_diesel_error)?;
        }
        Ok(())
    }

    async fn upsert_badges(
        &self,
        records: &[BadgeIngestion],
    ) -> Result<(), DescriptorIngestionRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;
        for record in records {
            let row = NewBadgeRow {
                id: record.id,
                slug: record.slug.as_str(),
                icon_key: record.icon_key.as_str(),
                localizations: &record.localizations,
            };
            diesel::insert_into(badges::table)
                .values(&row)
                .on_conflict(badges::id)
                .do_update()
                .set((
                    badges::slug.eq(excluded(badges::slug)),
                    badges::icon_key.eq(excluded(badges::icon_key)),
                    badges::localizations.eq(excluded(badges::localizations)),
                ))
                .execute(&mut conn)
                .await
                .map_err(map_diesel_error)?;
        }
        Ok(())
    }

    async fn upsert_safety_toggles(
        &self,
        records: &[SafetyToggleIngestion],
    ) -> Result<(), DescriptorIngestionRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;
        for record in records {
            let row = NewSafetyToggleRow {
                id: record.id,
                slug: record.slug.as_str(),
                icon_key: record.icon_key.as_str(),
                localizations: &record.localizations,
            };
            diesel::insert_into(safety_toggles::table)
                .values(&row)
                .on_conflict(safety_toggles::id)
                .do_update()
                .set((
                    safety_toggles::slug.eq(excluded(safety_toggles::slug)),
                    safety_toggles::icon_key.eq(excluded(safety_toggles::icon_key)),
                    safety_toggles::localizations.eq(excluded(safety_toggles::localizations)),
                ))
                .execute(&mut conn)
                .await
                .map_err(map_diesel_error)?;
        }
        Ok(())
    }

    async fn upsert_safety_presets(
        &self,
        records: &[SafetyPresetIngestion],
    ) -> Result<(), DescriptorIngestionRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;
        for record in records {
            let row = NewSafetyPresetRow {
                id: record.id,
                slug: record.slug.as_str(),
                icon_key: record.icon_key.as_str(),
                localizations: &record.localizations,
                safety_toggle_ids: record.safety_toggle_ids.as_slice(),
            };
            diesel::insert_into(safety_presets::table)
                .values(&row)
                .on_conflict(safety_presets::id)
                .do_update()
                .set((
                    safety_presets::slug.eq(excluded(safety_presets::slug)),
                    safety_presets::icon_key.eq(excluded(safety_presets::icon_key)),
                    safety_presets::localizations.eq(excluded(safety_presets::localizations)),
                    safety_presets::safety_toggle_ids
                        .eq(excluded(safety_presets::safety_toggle_ids)),
                ))
                .execute(&mut conn)
                .await
                .map_err(map_diesel_error)?;
        }
        Ok(())
    }

    async fn upsert_interest_themes(
        &self,
        records: &[InterestThemeIngestion],
    ) -> Result<(), DescriptorIngestionRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;
        for record in records {
            let row = NewInterestThemeRow {
                id: record.id,
                name: record.name.as_str(),
                description: record.description.as_deref(),
            };
            diesel::insert_into(interest_themes::table)
                .values(&row)
                .on_conflict(interest_themes::id)
                .do_update()
                .set((
                    interest_themes::name.eq(excluded(interest_themes::name)),
                    interest_themes::description.eq(excluded(interest_themes::description)),
                ))
                .execute(&mut conn)
                .await
                .map_err(map_diesel_error)?;
        }
        Ok(())
    }
}
