//! PostgreSQL-backed catalogue ingestion adapter.

use diesel::prelude::*;
use diesel_async::RunQueryDsl;

use crate::domain::ports::{
    CatalogueIngestionRepository, CatalogueIngestionRepositoryError, CommunityPickIngestion,
    RouteCategoryIngestion, RouteCollectionIngestion, RouteSummaryIngestion, ThemeIngestion,
    TrendingRouteHighlightIngestion,
};
use crate::impl_upsert_methods;

use super::diesel_helpers::{map_diesel_error_message, map_pool_error_message};
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
    CatalogueIngestionRepositoryError::connection(map_pool_error_message(error))
}

fn map_diesel_error(error: diesel::result::Error) -> CatalogueIngestionRepositoryError {
    CatalogueIngestionRepositoryError::query(map_diesel_error_message(
        error,
        "catalogue ingestion upsert",
    ))
}

impl<'a> From<&'a RouteCategoryIngestion> for NewRouteCategoryRow<'a> {
    fn from(record: &'a RouteCategoryIngestion) -> Self {
        Self {
            id: record.id,
            slug: record.slug.as_str(),
            icon_key: record.icon_key.as_str(),
            localizations: &record.localizations,
            route_count: record.route_count,
        }
    }
}

impl<'a> From<&'a ThemeIngestion> for NewThemeRow<'a> {
    fn from(record: &'a ThemeIngestion) -> Self {
        Self {
            id: record.id,
            slug: record.slug.as_str(),
            icon_key: record.icon_key.as_str(),
            localizations: &record.localizations,
            image: &record.image,
            walk_count: record.walk_count,
            distance_range_metres: record.distance_range_metres.as_slice(),
            rating: record.rating,
        }
    }
}

impl<'a> From<&'a RouteCollectionIngestion> for NewRouteCollectionRow<'a> {
    fn from(record: &'a RouteCollectionIngestion) -> Self {
        Self {
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
        }
    }
}

impl<'a> From<&'a RouteSummaryIngestion> for NewRouteSummaryRow<'a> {
    fn from(record: &'a RouteSummaryIngestion) -> Self {
        Self {
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
        }
    }
}

impl<'a> From<&'a TrendingRouteHighlightIngestion> for NewTrendingRouteHighlightRow<'a> {
    fn from(record: &'a TrendingRouteHighlightIngestion) -> Self {
        Self {
            id: record.id,
            route_summary_id: record.route_summary_id,
            trend_delta: record.trend_delta.as_str(),
            subtitle_localizations: &record.subtitle_localizations,
        }
    }
}

impl<'a> From<&'a CommunityPickIngestion> for NewCommunityPickRow<'a> {
    fn from(record: &'a CommunityPickIngestion) -> Self {
        Self {
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
        }
    }
}

impl_upsert_methods! {
    impl CatalogueIngestionRepository for DieselCatalogueIngestionRepository {
        error: CatalogueIngestionRepositoryError,
        map_pool_error: map_pool_error,
        map_diesel_error: map_diesel_error,
        pool: pool,
        methods: [
            (
                upsert_route_categories,
                RouteCategoryIngestion,
                NewRouteCategoryRow<'_>,
                route_categories,
                [slug, icon_key, localizations, route_count]
            ),
            (
                upsert_themes,
                ThemeIngestion,
                NewThemeRow<'_>,
                themes,
                [
                    slug,
                    icon_key,
                    localizations,
                    image,
                    walk_count,
                    distance_range_metres,
                    rating
                ]
            ),
            (
                upsert_route_collections,
                RouteCollectionIngestion,
                NewRouteCollectionRow<'_>,
                route_collections,
                [
                    slug,
                    icon_key,
                    localizations,
                    lead_image,
                    map_preview,
                    distance_range_metres,
                    duration_range_seconds,
                    difficulty,
                    route_ids
                ]
            ),
            (
                upsert_route_summaries,
                RouteSummaryIngestion,
                NewRouteSummaryRow<'_>,
                route_summaries,
                [
                    route_id,
                    category_id,
                    theme_id,
                    slug,
                    localizations,
                    hero_image,
                    distance_metres,
                    duration_seconds,
                    rating,
                    badge_ids,
                    difficulty,
                    interest_theme_ids
                ]
            ),
            (
                upsert_trending_highlights,
                TrendingRouteHighlightIngestion,
                NewTrendingRouteHighlightRow<'_>,
                trending_route_highlights,
                [route_summary_id, trend_delta, subtitle_localizations]
            ),
            (
                upsert_community_picks,
                CommunityPickIngestion,
                NewCommunityPickRow<'_>,
                community_picks,
                [
                    route_summary_id,
                    user_id,
                    localizations,
                    curator_display_name,
                    curator_avatar,
                    rating,
                    distance_metres,
                    duration_seconds,
                    saves
                ]
            )
        ],
        keep: {}
    }
}
