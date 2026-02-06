//! PostgreSQL-backed catalogue ingestion adapter.

use async_trait::async_trait;
use diesel::prelude::*;
use diesel::upsert::excluded;
use diesel_async::RunQueryDsl;
use tracing::debug;

use crate::domain::ports::{
    CatalogueIngestionRepository, CatalogueIngestionRepositoryError, CommunityPickIngestion,
    RouteCategoryIngestion, RouteCollectionIngestion, RouteSummaryIngestion, ThemeIngestion,
    TrendingRouteHighlightIngestion,
};

use super::models::{
    NewCommunityPickRow, NewRouteCategoryRow, NewRouteCollectionRow, NewRouteSummaryRow,
    NewThemeRow, NewTrendingRouteHighlightRow,
};
use super::pool::{DbPool, PoolError};
use super::schema::{
    community_picks, route_categories, route_collections, route_summaries, themes,
    trending_route_highlights,
};

/// Diesel-backed implementation of the catalogue ingestion port.
#[derive(Clone)]
pub struct DieselCatalogueIngestionRepository {
    pool: DbPool,
}

impl DieselCatalogueIngestionRepository {
    /// Create a new repository with the given connection pool.
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

fn map_pool_error(error: PoolError) -> CatalogueIngestionRepositoryError {
    match error {
        PoolError::Checkout { message } | PoolError::Build { message } => {
            CatalogueIngestionRepositoryError::connection(message)
        }
    }
}

fn map_diesel_error(error: diesel::result::Error) -> CatalogueIngestionRepositoryError {
    let error_message = error.to_string();
    debug!(%error_message, "catalogue diesel operation failed");
    CatalogueIngestionRepositoryError::query(error_message)
}

#[async_trait]
impl CatalogueIngestionRepository for DieselCatalogueIngestionRepository {
    async fn upsert_route_categories(
        &self,
        records: &[RouteCategoryIngestion],
    ) -> Result<(), CatalogueIngestionRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;
        for record in records {
            let row = NewRouteCategoryRow {
                id: record.id,
                slug: record.slug.as_str(),
                icon_key: record.icon_key.as_str(),
                localizations: &record.localizations,
                route_count: record.route_count,
            };
            diesel::insert_into(route_categories::table)
                .values(&row)
                .on_conflict(route_categories::id)
                .do_update()
                .set((
                    route_categories::slug.eq(excluded(route_categories::slug)),
                    route_categories::icon_key.eq(excluded(route_categories::icon_key)),
                    route_categories::localizations.eq(excluded(route_categories::localizations)),
                    route_categories::route_count.eq(excluded(route_categories::route_count)),
                ))
                .execute(&mut conn)
                .await
                .map_err(map_diesel_error)?;
        }
        Ok(())
    }

    async fn upsert_themes(
        &self,
        records: &[ThemeIngestion],
    ) -> Result<(), CatalogueIngestionRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;
        for record in records {
            let row = NewThemeRow {
                id: record.id,
                slug: record.slug.as_str(),
                icon_key: record.icon_key.as_str(),
                localizations: &record.localizations,
                image: &record.image,
                walk_count: record.walk_count,
                distance_range_metres: record.distance_range_metres.as_slice(),
                rating: record.rating,
            };
            diesel::insert_into(themes::table)
                .values(&row)
                .on_conflict(themes::id)
                .do_update()
                .set((
                    themes::slug.eq(excluded(themes::slug)),
                    themes::icon_key.eq(excluded(themes::icon_key)),
                    themes::localizations.eq(excluded(themes::localizations)),
                    themes::image.eq(excluded(themes::image)),
                    themes::walk_count.eq(excluded(themes::walk_count)),
                    themes::distance_range_metres.eq(excluded(themes::distance_range_metres)),
                    themes::rating.eq(excluded(themes::rating)),
                ))
                .execute(&mut conn)
                .await
                .map_err(map_diesel_error)?;
        }
        Ok(())
    }

    async fn upsert_route_collections(
        &self,
        records: &[RouteCollectionIngestion],
    ) -> Result<(), CatalogueIngestionRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;
        for record in records {
            let row = NewRouteCollectionRow {
                id: record.id,
                slug: record.slug.as_str(),
                icon_key: record.icon_key.as_str(),
                localizations: &record.localizations,
                lead_image: &record.lead_image,
                map_preview: &record.map_preview,
                distance_range_metres: record.distance_range_metres.as_slice(),
                duration_range_seconds: record.duration_range_seconds.as_slice(),
                difficulty: record.difficulty.as_str(),
                route_ids: record.route_ids.as_slice(),
            };
            diesel::insert_into(route_collections::table)
                .values(&row)
                .on_conflict(route_collections::id)
                .do_update()
                .set((
                    route_collections::slug.eq(excluded(route_collections::slug)),
                    route_collections::icon_key.eq(excluded(route_collections::icon_key)),
                    route_collections::localizations.eq(excluded(route_collections::localizations)),
                    route_collections::lead_image.eq(excluded(route_collections::lead_image)),
                    route_collections::map_preview.eq(excluded(route_collections::map_preview)),
                    route_collections::distance_range_metres
                        .eq(excluded(route_collections::distance_range_metres)),
                    route_collections::duration_range_seconds
                        .eq(excluded(route_collections::duration_range_seconds)),
                    route_collections::difficulty.eq(excluded(route_collections::difficulty)),
                    route_collections::route_ids.eq(excluded(route_collections::route_ids)),
                ))
                .execute(&mut conn)
                .await
                .map_err(map_diesel_error)?;
        }
        Ok(())
    }

    async fn upsert_route_summaries(
        &self,
        records: &[RouteSummaryIngestion],
    ) -> Result<(), CatalogueIngestionRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;
        for record in records {
            let row = NewRouteSummaryRow {
                id: record.id,
                route_id: record.route_id,
                category_id: record.category_id,
                theme_id: record.theme_id,
                slug: record.slug.as_deref(),
                localizations: &record.localizations,
                hero_image: &record.hero_image,
                distance_metres: record.distance_metres,
                duration_seconds: record.duration_seconds,
                rating: record.rating,
                badge_ids: record.badge_ids.as_slice(),
                difficulty: record.difficulty.as_str(),
                interest_theme_ids: record.interest_theme_ids.as_slice(),
            };
            diesel::insert_into(route_summaries::table)
                .values(&row)
                .on_conflict(route_summaries::id)
                .do_update()
                .set((
                    route_summaries::route_id.eq(excluded(route_summaries::route_id)),
                    route_summaries::category_id.eq(excluded(route_summaries::category_id)),
                    route_summaries::theme_id.eq(excluded(route_summaries::theme_id)),
                    route_summaries::slug.eq(excluded(route_summaries::slug)),
                    route_summaries::localizations.eq(excluded(route_summaries::localizations)),
                    route_summaries::hero_image.eq(excluded(route_summaries::hero_image)),
                    route_summaries::distance_metres.eq(excluded(route_summaries::distance_metres)),
                    route_summaries::duration_seconds
                        .eq(excluded(route_summaries::duration_seconds)),
                    route_summaries::rating.eq(excluded(route_summaries::rating)),
                    route_summaries::badge_ids.eq(excluded(route_summaries::badge_ids)),
                    route_summaries::difficulty.eq(excluded(route_summaries::difficulty)),
                    route_summaries::interest_theme_ids
                        .eq(excluded(route_summaries::interest_theme_ids)),
                ))
                .execute(&mut conn)
                .await
                .map_err(map_diesel_error)?;
        }
        Ok(())
    }

    async fn upsert_trending_highlights(
        &self,
        records: &[TrendingRouteHighlightIngestion],
    ) -> Result<(), CatalogueIngestionRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;
        for record in records {
            let row = NewTrendingRouteHighlightRow {
                id: record.id,
                route_summary_id: record.route_summary_id,
                trend_delta: record.trend_delta.as_str(),
                subtitle_localizations: &record.subtitle_localizations,
            };
            diesel::insert_into(trending_route_highlights::table)
                .values(&row)
                .on_conflict(trending_route_highlights::id)
                .do_update()
                .set((
                    trending_route_highlights::route_summary_id
                        .eq(excluded(trending_route_highlights::route_summary_id)),
                    trending_route_highlights::trend_delta
                        .eq(excluded(trending_route_highlights::trend_delta)),
                    trending_route_highlights::subtitle_localizations
                        .eq(excluded(trending_route_highlights::subtitle_localizations)),
                ))
                .execute(&mut conn)
                .await
                .map_err(map_diesel_error)?;
        }
        Ok(())
    }

    async fn upsert_community_picks(
        &self,
        records: &[CommunityPickIngestion],
    ) -> Result<(), CatalogueIngestionRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;
        for record in records {
            let row = NewCommunityPickRow {
                id: record.id,
                route_summary_id: record.route_summary_id,
                user_id: record.user_id,
                localizations: &record.localizations,
                curator_display_name: record.curator_display_name.as_str(),
                curator_avatar: &record.curator_avatar,
                rating: record.rating,
                distance_metres: record.distance_metres,
                duration_seconds: record.duration_seconds,
                saves: record.saves,
            };
            diesel::insert_into(community_picks::table)
                .values(&row)
                .on_conflict(community_picks::id)
                .do_update()
                .set((
                    community_picks::route_summary_id
                        .eq(excluded(community_picks::route_summary_id)),
                    community_picks::user_id.eq(excluded(community_picks::user_id)),
                    community_picks::localizations.eq(excluded(community_picks::localizations)),
                    community_picks::curator_display_name
                        .eq(excluded(community_picks::curator_display_name)),
                    community_picks::curator_avatar.eq(excluded(community_picks::curator_avatar)),
                    community_picks::rating.eq(excluded(community_picks::rating)),
                    community_picks::distance_metres.eq(excluded(community_picks::distance_metres)),
                    community_picks::duration_seconds
                        .eq(excluded(community_picks::duration_seconds)),
                    community_picks::saves.eq(excluded(community_picks::saves)),
                ))
                .execute(&mut conn)
                .await
                .map_err(map_diesel_error)?;
        }
        Ok(())
    }
}
