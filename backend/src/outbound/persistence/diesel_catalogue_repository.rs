//! PostgreSQL-backed catalogue read adapter.

use async_trait::async_trait;
use chrono::Utc;
use diesel::prelude::*;
use diesel_async::AsyncConnection as _;
use diesel_async::scoped_futures::ScopedFutureExt as _;
use diesel_async::{AsyncPgConnection, RunQueryDsl};

use crate::domain::ports::{
    CatalogueRepository, CatalogueRepositoryError, ExploreCatalogueSnapshot,
};
use crate::domain::{
    CommunityPick, CommunityPickDraft, RouteCategory, RouteCategoryDraft, RouteCollection,
    RouteCollectionDraft, RouteSummary, RouteSummaryDraft, Theme, ThemeDraft,
    TrendingRouteHighlight,
};

use super::diesel_helpers::{collect_rows, map_diesel_error_message, map_pool_error_message};
use super::json_serializers::{
    json_to_image_asset, json_to_localization_map, json_to_semantic_icon_identifier,
};
use super::models::{
    CommunityPickRow, RouteCategoryRow, RouteCollectionRow, RouteSummaryRow, ThemeRow,
    TrendingRouteHighlightRow,
};
use super::pool::{DbPool, PoolError};
use super::schema::{
    community_picks, route_categories, route_collections, route_summaries, themes,
    trending_route_highlights,
};

/// Diesel-backed implementation of the catalogue read port.
#[derive(Clone)]
pub struct DieselCatalogueRepository {
    pool: DbPool,
}

impl DieselCatalogueRepository {
    /// Create a new repository with the given connection pool.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let config = PoolConfig::new("postgres://localhost/mydb");
    /// let pool = DbPool::new(config).await.unwrap();
    /// let repo = DieselCatalogueRepository::new(pool);
    /// ```
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

fn map_pool_error(error: PoolError) -> CatalogueRepositoryError {
    CatalogueRepositoryError::connection(map_pool_error_message(error))
}

fn map_diesel_error(error: diesel::result::Error) -> CatalogueRepositoryError {
    CatalogueRepositoryError::query(map_diesel_error_message(error, "catalogue read"))
}

// ---------------------------------------------------------------------------
// Row-to-domain converters
// ---------------------------------------------------------------------------

fn row_to_route_category(row: RouteCategoryRow) -> Result<RouteCategory, String> {
    let localizations = json_to_localization_map(&row.localizations)?;
    let icon_key = json_to_semantic_icon_identifier(&row.icon_key)?;
    RouteCategory::new(RouteCategoryDraft {
        id: row.id,
        slug: row.slug,
        icon_key,
        localizations,
        route_count: row.route_count,
    })
    .map_err(|e| e.to_string())
}

fn row_to_theme(row: ThemeRow) -> Result<crate::domain::Theme, String> {
    let localizations = json_to_localization_map(&row.localizations)?;
    let icon_key = json_to_semantic_icon_identifier(&row.icon_key)?;
    let image = json_to_image_asset(&row.image)?;
    let distance_range_metres =
        vec_to_pair(&row.distance_range_metres, "theme.distance_range_metres")?;
    ThemeDraft {
        id: row.id,
        slug: row.slug,
        icon_key,
        localizations,
        image,
        walk_count: row.walk_count,
        distance_range_metres,
        rating: row.rating,
    }
    .try_into()
    .map_err(|e: crate::domain::CatalogueValidationError| e.to_string())
}

fn row_to_route_collection(
    row: RouteCollectionRow,
) -> Result<crate::domain::RouteCollection, String> {
    let localizations = json_to_localization_map(&row.localizations)?;
    let icon_key = json_to_semantic_icon_identifier(&row.icon_key)?;
    let lead_image = json_to_image_asset(&row.lead_image)?;
    let map_preview = json_to_image_asset(&row.map_preview)?;
    let distance_range_metres = vec_to_pair(
        &row.distance_range_metres,
        "route_collection.distance_range_metres",
    )?;
    let duration_range_seconds = vec_to_pair(
        &row.duration_range_seconds,
        "route_collection.duration_range_seconds",
    )?;
    RouteCollectionDraft {
        id: row.id,
        slug: row.slug,
        icon_key,
        localizations,
        lead_image,
        map_preview,
        distance_range_metres,
        duration_range_seconds,
        difficulty: row.difficulty,
        route_ids: row.route_ids,
    }
    .try_into()
    .map_err(|e: crate::domain::CatalogueValidationError| e.to_string())
}

fn row_to_route_summary(row: RouteSummaryRow) -> Result<crate::domain::RouteSummary, String> {
    let localizations = json_to_localization_map(&row.localizations)?;
    let hero_image = json_to_image_asset(&row.hero_image)?;
    RouteSummaryDraft {
        id: row.id,
        route_id: row.route_id,
        category_id: row.category_id,
        theme_id: row.theme_id,
        slug: row.slug,
        localizations,
        hero_image,
        distance_metres: row.distance_metres,
        duration_seconds: row.duration_seconds,
        rating: row.rating,
        badge_ids: row.badge_ids,
        difficulty: row.difficulty,
        interest_theme_ids: row.interest_theme_ids,
    }
    .try_into()
    .map_err(|e: crate::domain::CatalogueValidationError| e.to_string())
}

fn row_to_trending_highlight(
    row: TrendingRouteHighlightRow,
) -> Result<crate::domain::TrendingRouteHighlight, String> {
    let subtitle_localizations = json_to_localization_map(&row.subtitle_localizations)?;
    crate::domain::TrendingRouteHighlight::new(
        row.id,
        row.route_summary_id,
        row.trend_delta,
        subtitle_localizations,
    )
    .map_err(|e| e.to_string())
}

fn row_to_community_pick(row: CommunityPickRow) -> Result<crate::domain::CommunityPick, String> {
    let localizations = json_to_localization_map(&row.localizations)?;
    let curator_avatar = json_to_image_asset(&row.curator_avatar)?;
    crate::domain::CommunityPick::new(CommunityPickDraft {
        id: row.id,
        route_summary_id: row.route_summary_id,
        user_id: row.user_id,
        localizations,
        curator_display_name: row.curator_display_name,
        curator_avatar,
        rating: row.rating,
        distance_metres: row.distance_metres,
        duration_seconds: row.duration_seconds,
        saves: row.saves,
    })
    .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn vec_to_pair(v: &[i32], field: &str) -> Result<[i32; 2], String> {
    v.try_into()
        .map_err(|_| format!("{field}: expected exactly 2 elements, got {}", v.len()))
}

// ---------------------------------------------------------------------------
// Trait implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl CatalogueRepository for DieselCatalogueRepository {
    async fn explore_snapshot(&self) -> Result<ExploreCatalogueSnapshot, CatalogueRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;
        let rows = self.fetch_catalogue_rows(&mut conn).await?;
        let (categories, themes, collections, routes, trending, community_pick) =
            Self::convert_catalogue_rows(rows)?;

        Ok(ExploreCatalogueSnapshot {
            generated_at: Utc::now(),
            categories,
            routes,
            themes,
            collections,
            trending,
            community_pick,
        })
    }
}

type CatalogueRows = (
    Vec<RouteCategoryRow>,
    Vec<ThemeRow>,
    Vec<RouteCollectionRow>,
    Vec<RouteSummaryRow>,
    Vec<TrendingRouteHighlightRow>,
    Option<CommunityPickRow>,
);

type ConvertedCatalogueRows = (
    Vec<RouteCategory>,
    Vec<Theme>,
    Vec<RouteCollection>,
    Vec<RouteSummary>,
    Vec<TrendingRouteHighlight>,
    Option<CommunityPick>,
);

impl DieselCatalogueRepository {
    /// Fetch all catalogue rows in a single transaction so all SELECTs observe
    /// a consistent MVCC snapshot, preventing mixed-version results during
    /// concurrent ingestion.
    async fn fetch_catalogue_rows(
        &self,
        conn: &mut AsyncPgConnection,
    ) -> Result<CatalogueRows, CatalogueRepositoryError> {
        conn.transaction(|conn| {
            async move {
                let cats: Vec<RouteCategoryRow> = route_categories::table
                    .select(RouteCategoryRow::as_select())
                    .order_by(route_categories::slug)
                    .load(conn)
                    .await?;
                let themes: Vec<ThemeRow> = themes::table
                    .select(ThemeRow::as_select())
                    .order_by(themes::slug)
                    .load(conn)
                    .await?;
                let colls: Vec<RouteCollectionRow> = route_collections::table
                    .select(RouteCollectionRow::as_select())
                    .order_by(route_collections::slug)
                    .load(conn)
                    .await?;
                let sums: Vec<RouteSummaryRow> = route_summaries::table
                    .select(RouteSummaryRow::as_select())
                    .order_by((route_summaries::slug, route_summaries::id))
                    .load(conn)
                    .await?;
                let trends: Vec<TrendingRouteHighlightRow> = trending_route_highlights::table
                    .select(TrendingRouteHighlightRow::as_select())
                    .order_by(trending_route_highlights::highlighted_at.desc())
                    .load(conn)
                    .await?;
                let pick: Option<CommunityPickRow> = community_picks::table
                    .select(CommunityPickRow::as_select())
                    .order_by(community_picks::picked_at.desc())
                    .first(conn)
                    .await
                    .optional()?;
                Ok((cats, themes, colls, sums, trends, pick))
            }
            .scope_boxed()
        })
        .await
        .map_err(map_diesel_error)
    }

    fn convert_catalogue_rows(
        rows: CatalogueRows,
    ) -> Result<ConvertedCatalogueRows, CatalogueRepositoryError> {
        let (category_rows, theme_rows, collection_rows, summary_rows, trending_rows, pick_row) =
            rows;
        let map_err = CatalogueRepositoryError::query;
        let categories = collect_rows(
            category_rows.into_iter().map(row_to_route_category),
            map_err,
        )?;
        let themes = collect_rows(theme_rows.into_iter().map(row_to_theme), map_err)?;
        let collections = collect_rows(
            collection_rows.into_iter().map(row_to_route_collection),
            map_err,
        )?;
        let routes = collect_rows(summary_rows.into_iter().map(row_to_route_summary), map_err)?;
        let trending = collect_rows(
            trending_rows.into_iter().map(row_to_trending_highlight),
            map_err,
        )?;
        let community_pick = pick_row
            .map(row_to_community_pick)
            .transpose()
            .map_err(CatalogueRepositoryError::query)?;
        Ok((
            categories,
            themes,
            collections,
            routes,
            trending,
            community_pick,
        ))
    }
}
