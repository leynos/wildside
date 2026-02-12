//! PostgreSQL-backed catalogue ingestion adapter.

use diesel::prelude::*;
use diesel_async::RunQueryDsl;

use crate::domain::ports::{CatalogueIngestionRepository, CatalogueIngestionRepositoryError};
use crate::domain::{
    CommunityPick, RouteCategory, RouteCollection, RouteSummary, Theme, TrendingRouteHighlight,
};
use crate::impl_upsert_methods;

use super::diesel_helpers::{map_diesel_error_message, map_pool_error_message};
use super::json_serializers::{image_asset_to_json, localization_map_to_json};
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

impl<'a> From<&'a RouteCategory> for NewRouteCategoryRow<'a> {
    fn from(record: &'a RouteCategory) -> Self {
        Self {
            id: record.id(),
            slug: record.slug(),
            icon_key: record.icon_key().as_ref(),
            localizations: localization_map_to_json(record.localizations()),
            route_count: record.route_count(),
        }
    }
}

impl<'a> From<&'a Theme> for NewThemeRow<'a> {
    fn from(record: &'a Theme) -> Self {
        Self {
            id: record.id(),
            slug: record.slug(),
            icon_key: record.icon_key().as_ref(),
            localizations: localization_map_to_json(record.localizations()),
            image: image_asset_to_json(record.image()),
            walk_count: record.walk_count(),
            distance_range_metres: record.distance_range_metres().as_slice(),
            rating: record.rating(),
        }
    }
}

impl<'a> From<&'a RouteCollection> for NewRouteCollectionRow<'a> {
    fn from(record: &'a RouteCollection) -> Self {
        Self {
            id: record.id(),
            slug: record.slug(),
            icon_key: record.icon_key().as_ref(),
            localizations: localization_map_to_json(record.localizations()),
            lead_image: image_asset_to_json(record.lead_image()),
            map_preview: image_asset_to_json(record.map_preview()),
            distance_range_metres: record.distance_range_metres().as_slice(),
            duration_range_seconds: record.duration_range_seconds().as_slice(),
            difficulty: record.difficulty(),
            route_ids: record.route_ids(),
        }
    }
}

impl<'a> From<&'a RouteSummary> for NewRouteSummaryRow<'a> {
    fn from(record: &'a RouteSummary) -> Self {
        Self {
            id: record.id(),
            route_id: record.route_id(),
            category_id: record.category_id(),
            theme_id: record.theme_id(),
            slug: record.slug(),
            localizations: localization_map_to_json(record.localizations()),
            hero_image: image_asset_to_json(record.hero_image()),
            distance_metres: record.distance_metres(),
            duration_seconds: record.duration_seconds(),
            rating: record.rating(),
            badge_ids: record.badge_ids(),
            difficulty: record.difficulty(),
            interest_theme_ids: record.interest_theme_ids(),
        }
    }
}

impl<'a> From<&'a TrendingRouteHighlight> for NewTrendingRouteHighlightRow<'a> {
    fn from(record: &'a TrendingRouteHighlight) -> Self {
        Self {
            id: record.id(),
            route_summary_id: record.route_summary_id(),
            trend_delta: record.trend_delta(),
            subtitle_localizations: localization_map_to_json(record.subtitle_localizations()),
        }
    }
}

impl<'a> From<&'a CommunityPick> for NewCommunityPickRow<'a> {
    fn from(record: &'a CommunityPick) -> Self {
        Self {
            id: record.id(),
            route_summary_id: record.route_summary_id(),
            user_id: record.user_id(),
            localizations: localization_map_to_json(record.localizations()),
            curator_display_name: record.curator_display_name(),
            curator_avatar: image_asset_to_json(record.curator_avatar()),
            rating: record.rating(),
            distance_metres: record.distance_metres(),
            duration_seconds: record.duration_seconds(),
            saves: record.saves(),
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
                RouteCategory,
                NewRouteCategoryRow<'_>,
                route_categories,
                [slug, icon_key, localizations, route_count]
            ),
            (
                upsert_themes,
                Theme,
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
                RouteCollection,
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
                RouteSummary,
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
                TrendingRouteHighlight,
                NewTrendingRouteHighlightRow<'_>,
                trending_route_highlights,
                [route_summary_id, trend_delta, subtitle_localizations]
            ),
            (
                upsert_community_picks,
                CommunityPick,
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
