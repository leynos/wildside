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

macro_rules! impl_upsert_descriptor {
    (
        $method_name:ident,
        $ingestion_type:ty,
        $row_type:ident,
        $table:ident,
        [$($field:ident),+ $(,)?],
        $pool:expr,
        $records:expr
    ) => {{
        let _method_name = stringify!($method_name);
        let typed_records: &[$ingestion_type] = $records;
        let mut conn = $pool.get().await.map_err(map_pool_error)?;
        for record in typed_records {
            let row = $row_type::from(record);
            diesel::insert_into($table::table)
                .values(&row)
                .on_conflict($table::id)
                .do_update()
                .set((
                    $(
                        $table::$field.eq(excluded($table::$field)),
                    )+
                ))
                .execute(&mut conn)
                .await
                .map_err(map_diesel_error)?;
        }
        Ok(())
    }};
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

#[async_trait]
impl DescriptorIngestionRepository for DieselDescriptorIngestionRepository {
    async fn upsert_tags(
        &self,
        records: &[TagIngestion],
    ) -> Result<(), DescriptorIngestionRepositoryError> {
        impl_upsert_descriptor!(
            upsert_tags,
            TagIngestion,
            NewTagRow,
            tags,
            [slug, icon_key, localizations],
            self.pool,
            records
        )
    }

    async fn upsert_badges(
        &self,
        records: &[BadgeIngestion],
    ) -> Result<(), DescriptorIngestionRepositoryError> {
        impl_upsert_descriptor!(
            upsert_badges,
            BadgeIngestion,
            NewBadgeRow,
            badges,
            [slug, icon_key, localizations],
            self.pool,
            records
        )
    }

    async fn upsert_safety_toggles(
        &self,
        records: &[SafetyToggleIngestion],
    ) -> Result<(), DescriptorIngestionRepositoryError> {
        impl_upsert_descriptor!(
            upsert_safety_toggles,
            SafetyToggleIngestion,
            NewSafetyToggleRow,
            safety_toggles,
            [slug, icon_key, localizations],
            self.pool,
            records
        )
    }

    async fn upsert_safety_presets(
        &self,
        records: &[SafetyPresetIngestion],
    ) -> Result<(), DescriptorIngestionRepositoryError> {
        impl_upsert_descriptor!(
            upsert_safety_presets,
            SafetyPresetIngestion,
            NewSafetyPresetRow,
            safety_presets,
            [slug, icon_key, localizations, safety_toggle_ids],
            self.pool,
            records
        )
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
