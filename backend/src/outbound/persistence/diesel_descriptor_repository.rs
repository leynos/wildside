//! PostgreSQL-backed descriptor read adapter.

use async_trait::async_trait;
use chrono::Utc;
use diesel::prelude::*;
use diesel_async::AsyncConnection as _;
use diesel_async::RunQueryDsl;
use diesel_async::scoped_futures::ScopedFutureExt as _;

use crate::domain::ports::{DescriptorRepository, DescriptorRepositoryError, DescriptorSnapshot};
use crate::domain::{Badge, InterestTheme, SafetyPreset, SafetyPresetDraft, SafetyToggle, Tag};

use super::diesel_helpers::{collect_rows, map_diesel_error_message, map_pool_error_message};
use super::json_serializers::{json_to_localization_map, json_to_semantic_icon_identifier};
use super::models::{BadgeRow, InterestThemeRow, SafetyPresetRow, SafetyToggleRow, TagRow};
use super::pool::{DbPool, PoolError};
use super::schema::{badges, interest_themes, safety_presets, safety_toggles, tags};

/// Diesel-backed implementation of the descriptor read port.
#[derive(Clone)]
pub struct DieselDescriptorRepository {
    pool: DbPool,
}

impl DieselDescriptorRepository {
    /// Create a new repository with the given connection pool.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let config = PoolConfig::new("postgres://localhost/mydb");
    /// let pool = DbPool::new(config).await.unwrap();
    /// let repo = DieselDescriptorRepository::new(pool);
    /// ```
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

fn map_pool_error(error: PoolError) -> DescriptorRepositoryError {
    DescriptorRepositoryError::connection(map_pool_error_message(error))
}

fn map_diesel_error(error: diesel::result::Error) -> DescriptorRepositoryError {
    DescriptorRepositoryError::query(map_diesel_error_message(error, "descriptor read"))
}

// ---------------------------------------------------------------------------
// Row-to-domain converters
// ---------------------------------------------------------------------------

fn row_to_tag(row: TagRow) -> Result<Tag, String> {
    let localizations = json_to_localization_map(row.localizations)?;
    let icon_key = json_to_semantic_icon_identifier(&row.icon_key)?;
    Tag::new(row.id, row.slug, icon_key, localizations).map_err(|e| e.to_string())
}

fn row_to_badge(row: BadgeRow) -> Result<Badge, String> {
    let localizations = json_to_localization_map(row.localizations)?;
    let icon_key = json_to_semantic_icon_identifier(&row.icon_key)?;
    Badge::new(row.id, row.slug, icon_key, localizations).map_err(|e| e.to_string())
}

fn row_to_safety_toggle(row: SafetyToggleRow) -> Result<SafetyToggle, String> {
    let localizations = json_to_localization_map(row.localizations)?;
    let icon_key = json_to_semantic_icon_identifier(&row.icon_key)?;
    SafetyToggle::new(row.id, row.slug, icon_key, localizations).map_err(|e| e.to_string())
}

fn row_to_safety_preset(row: SafetyPresetRow) -> Result<SafetyPreset, String> {
    let localizations = json_to_localization_map(row.localizations)?;
    let icon_key = json_to_semantic_icon_identifier(&row.icon_key)?;
    SafetyPreset::new(SafetyPresetDraft {
        id: row.id,
        slug: row.slug,
        icon_key,
        localizations,
        safety_toggle_ids: row.safety_toggle_ids,
    })
    .map_err(|e| e.to_string())
}

fn row_to_interest_theme(row: InterestThemeRow) -> InterestTheme {
    InterestTheme::new(row.id, row.name, row.description)
}

// ---------------------------------------------------------------------------
// Trait implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl DescriptorRepository for DieselDescriptorRepository {
    async fn descriptor_snapshot(&self) -> Result<DescriptorSnapshot, DescriptorRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        // Read all tables in a single transaction so all SELECTs observe
        // a consistent MVCC snapshot, preventing mixed-version results
        // during concurrent ingestion.
        let (tag_rows, badge_rows, toggle_rows, preset_rows, theme_rows) = conn
            .transaction(|conn| {
                async move {
                    let tags: Vec<TagRow> = tags::table
                        .select(TagRow::as_select())
                        .order_by(tags::slug)
                        .load(conn)
                        .await?;
                    let badges: Vec<BadgeRow> = badges::table
                        .select(BadgeRow::as_select())
                        .order_by(badges::slug)
                        .load(conn)
                        .await?;
                    let toggles: Vec<SafetyToggleRow> = safety_toggles::table
                        .select(SafetyToggleRow::as_select())
                        .order_by(safety_toggles::slug)
                        .load(conn)
                        .await?;
                    let presets: Vec<SafetyPresetRow> = safety_presets::table
                        .select(SafetyPresetRow::as_select())
                        .order_by(safety_presets::slug)
                        .load(conn)
                        .await?;
                    let themes: Vec<InterestThemeRow> = interest_themes::table
                        .select(InterestThemeRow::as_select())
                        .order_by(interest_themes::name)
                        .load(conn)
                        .await?;
                    Ok((tags, badges, toggles, presets, themes))
                }
                .scope_boxed()
            })
            .await
            .map_err(map_diesel_error)?;

        let map_err = DescriptorRepositoryError::query;
        Ok(DescriptorSnapshot {
            generated_at: Utc::now(),
            tags: collect_rows(tag_rows.into_iter().map(row_to_tag), map_err)?,
            badges: collect_rows(badge_rows.into_iter().map(row_to_badge), map_err)?,
            safety_toggles: collect_rows(
                toggle_rows.into_iter().map(row_to_safety_toggle),
                map_err,
            )?,
            safety_presets: collect_rows(
                preset_rows.into_iter().map(row_to_safety_preset),
                map_err,
            )?,
            interest_themes: theme_rows.into_iter().map(row_to_interest_theme).collect(),
        })
    }
}
