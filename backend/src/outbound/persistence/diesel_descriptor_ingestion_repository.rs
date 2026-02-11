//! PostgreSQL-backed descriptor ingestion adapter.

use diesel::prelude::*;
use diesel_async::RunQueryDsl;

use crate::domain::ports::{
    DescriptorIngestionRepository, DescriptorIngestionRepositoryError, InterestThemeIngestion,
};
use crate::domain::{Badge, SafetyPreset, SafetyToggle, Tag};
use crate::impl_upsert_methods;

use super::diesel_helpers::{map_diesel_error_message, map_pool_error_message};
use super::json_serializers::localization_map_to_json;
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
    DescriptorIngestionRepositoryError::connection(map_pool_error_message(error))
}

fn map_diesel_error(error: diesel::result::Error) -> DescriptorIngestionRepositoryError {
    DescriptorIngestionRepositoryError::query(map_diesel_error_message(
        error,
        "descriptor ingestion upsert",
    ))
}

impl From<&Tag> for NewTagRow {
    fn from(record: &Tag) -> Self {
        Self {
            id: record.id,
            slug: record.slug.clone(),
            icon_key: record.icon_key.as_ref().to_owned(),
            localizations: localization_map_to_json(&record.localizations),
        }
    }
}

impl From<&Badge> for NewBadgeRow {
    fn from(record: &Badge) -> Self {
        Self {
            id: record.id,
            slug: record.slug.clone(),
            icon_key: record.icon_key.as_ref().to_owned(),
            localizations: localization_map_to_json(&record.localizations),
        }
    }
}

impl From<&SafetyToggle> for NewSafetyToggleRow {
    fn from(record: &SafetyToggle) -> Self {
        Self {
            id: record.id,
            slug: record.slug.clone(),
            icon_key: record.icon_key.as_ref().to_owned(),
            localizations: localization_map_to_json(&record.localizations),
        }
    }
}

impl From<&SafetyPreset> for NewSafetyPresetRow {
    fn from(record: &SafetyPreset) -> Self {
        Self {
            id: record.id,
            slug: record.slug.clone(),
            icon_key: record.icon_key.as_ref().to_owned(),
            localizations: localization_map_to_json(&record.localizations),
            safety_toggle_ids: record.safety_toggle_ids.clone(),
        }
    }
}

impl From<&InterestThemeIngestion> for NewInterestThemeRow {
    fn from(record: &InterestThemeIngestion) -> Self {
        Self {
            id: record.id,
            name: record.name.clone(),
            description: record.description.clone(),
        }
    }
}

impl_upsert_methods! {
    impl DescriptorIngestionRepository for DieselDescriptorIngestionRepository {
        error: DescriptorIngestionRepositoryError,
        map_pool_error: map_pool_error,
        map_diesel_error: map_diesel_error,
        pool: pool,
        methods: [
            (
                upsert_tags,
                Tag,
                NewTagRow,
                tags,
                [slug, icon_key, localizations]
            ),
            (
                upsert_badges,
                Badge,
                NewBadgeRow,
                badges,
                [slug, icon_key, localizations]
            ),
            (
                upsert_safety_toggles,
                SafetyToggle,
                NewSafetyToggleRow,
                safety_toggles,
                [slug, icon_key, localizations]
            ),
            (
                upsert_safety_presets,
                SafetyPreset,
                NewSafetyPresetRow,
                safety_presets,
                [slug, icon_key, localizations, safety_toggle_ids]
            )
        ],
        keep: {
            async fn upsert_interest_themes(
                &self,
                records: &[InterestThemeIngestion],
            ) -> Result<(), DescriptorIngestionRepositoryError> {
                use diesel_async::AsyncConnection as _;
                use diesel_async::scoped_futures::ScopedFutureExt as _;

                if records.is_empty() {
                    return Ok(());
                }
                let mut conn = self.pool.get().await.map_err(map_pool_error)?;
                let rows: Vec<NewInterestThemeRow> = records
                    .iter()
                    .map(NewInterestThemeRow::from)
                    .collect();

                conn.transaction(|conn| {
                    async move {
                        diesel::insert_into(interest_themes::table)
                            .values(&rows)
                            .on_conflict(interest_themes::id)
                            .do_update()
                            .set((
                                interest_themes::name
                                    .eq(diesel::upsert::excluded(interest_themes::name)),
                                interest_themes::description
                                    .eq(diesel::upsert::excluded(interest_themes::description)),
                            ))
                            .execute(conn)
                            .await?;
                        Ok(())
                    }
                    .scope_boxed()
                })
                .await
                .map_err(map_diesel_error)?;
                Ok(())
            }
        }
    }
}
