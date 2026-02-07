//! PostgreSQL-backed descriptor ingestion adapter.

use diesel::prelude::*;
use diesel_async::RunQueryDsl;

use crate::domain::ports::{
    BadgeIngestion, DescriptorIngestionRepository, DescriptorIngestionRepositoryError,
    InterestThemeIngestion, SafetyPresetIngestion, SafetyToggleIngestion, TagIngestion,
};
use crate::impl_upsert_methods;

use super::diesel_helpers::{map_diesel_error_message, map_pool_error_message};
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
    #[rustfmt::skip]
    pub fn new(pool: DbPool) -> Self { Self { pool } }
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

impl<'a> From<&'a TagIngestion> for NewTagRow<'a> {
    fn from(record: &'a TagIngestion) -> Self {
        Self {
            id: record.id,
            slug: record.slug.as_str(),
            icon_key: record.icon_key.as_str(),
            localizations: &record.localizations,
        }
    }
}

impl<'a> From<&'a BadgeIngestion> for NewBadgeRow<'a> {
    fn from(record: &'a BadgeIngestion) -> Self {
        Self {
            id: record.id,
            slug: record.slug.as_str(),
            icon_key: record.icon_key.as_str(),
            localizations: &record.localizations,
        }
    }
}

impl<'a> From<&'a SafetyToggleIngestion> for NewSafetyToggleRow<'a> {
    fn from(record: &'a SafetyToggleIngestion) -> Self {
        Self {
            id: record.id,
            slug: record.slug.as_str(),
            icon_key: record.icon_key.as_str(),
            localizations: &record.localizations,
        }
    }
}

impl<'a> From<&'a SafetyPresetIngestion> for NewSafetyPresetRow<'a> {
    fn from(record: &'a SafetyPresetIngestion) -> Self {
        Self {
            id: record.id,
            slug: record.slug.as_str(),
            icon_key: record.icon_key.as_str(),
            localizations: &record.localizations,
            safety_toggle_ids: record.safety_toggle_ids.as_slice(),
        }
    }
}

impl<'a> From<&'a InterestThemeIngestion> for NewInterestThemeRow<'a> {
    fn from(record: &'a InterestThemeIngestion) -> Self {
        Self {
            id: record.id,
            name: record.name.as_str(),
            description: record.description.as_deref(),
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
                TagIngestion,
                NewTagRow<'_>,
                tags,
                [slug, icon_key, localizations]
            ),
            (
                upsert_badges,
                BadgeIngestion,
                NewBadgeRow<'_>,
                badges,
                [slug, icon_key, localizations]
            ),
            (
                upsert_safety_toggles,
                SafetyToggleIngestion,
                NewSafetyToggleRow<'_>,
                safety_toggles,
                [slug, icon_key, localizations]
            ),
            (
                upsert_safety_presets,
                SafetyPresetIngestion,
                NewSafetyPresetRow<'_>,
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
                let rows: Vec<NewInterestThemeRow<'_>> = records
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
