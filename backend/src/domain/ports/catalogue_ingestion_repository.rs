//! Port abstraction for catalogue ingestion writes.
//!
//! This port keeps catalogue materialisation behind the domain boundary so
//! ingestion jobs can evolve without leaking persistence details.

use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

use super::define_port_error;

define_port_error! {
    /// Errors raised when persisting catalogue ingestion payloads.
    pub enum CatalogueIngestionRepositoryError {
        /// Repository connection could not be established.
        Connection { message: String } =>
            "catalogue ingestion connection failed: {message}",
        /// Query or mutation failed during execution.
        Query { message: String } =>
            "catalogue ingestion query failed: {message}",
    }
}

/// Ingestion payload for route categories.
#[derive(Debug, Clone)]
pub struct RouteCategoryIngestion {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: String,
    pub localizations: Value,
    pub route_count: i32,
}

/// Ingestion payload for themes.
#[derive(Debug, Clone)]
pub struct ThemeIngestion {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: String,
    pub localizations: Value,
    pub image: Value,
    pub walk_count: i32,
    pub distance_range_metres: Vec<i32>,
    pub rating: f32,
}

/// Ingestion payload for route collections.
#[derive(Debug, Clone)]
pub struct RouteCollectionIngestion {
    pub id: Uuid,
    pub slug: String,
    pub icon_key: String,
    pub localizations: Value,
    pub lead_image: Value,
    pub map_preview: Value,
    pub distance_range_metres: Vec<i32>,
    pub duration_range_seconds: Vec<i32>,
    pub difficulty: String,
    pub route_ids: Vec<Uuid>,
}

/// Ingestion payload for route summaries.
#[derive(Debug, Clone)]
pub struct RouteSummaryIngestion {
    pub id: Uuid,
    pub route_id: Uuid,
    pub category_id: Uuid,
    pub theme_id: Uuid,
    pub slug: Option<String>,
    pub localizations: Value,
    pub hero_image: Value,
    pub distance_metres: i32,
    pub duration_seconds: i32,
    pub rating: f32,
    pub badge_ids: Vec<Uuid>,
    pub difficulty: String,
    pub interest_theme_ids: Vec<Uuid>,
}

/// Ingestion payload for trending overlays.
#[derive(Debug, Clone)]
pub struct TrendingRouteHighlightIngestion {
    pub id: Uuid,
    pub route_summary_id: Uuid,
    pub trend_delta: String,
    pub subtitle_localizations: Value,
}

/// Ingestion payload for community picks.
#[derive(Debug, Clone)]
pub struct CommunityPickIngestion {
    pub id: Uuid,
    pub route_summary_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub localizations: Value,
    pub curator_display_name: String,
    pub curator_avatar: Value,
    pub rating: f32,
    pub distance_metres: i32,
    pub duration_seconds: i32,
    pub saves: i32,
}

/// Port for writing catalogue snapshots.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait CatalogueIngestionRepository: Send + Sync {
    async fn upsert_route_categories(
        &self,
        records: &[RouteCategoryIngestion],
    ) -> Result<(), CatalogueIngestionRepositoryError>;

    async fn upsert_themes(
        &self,
        records: &[ThemeIngestion],
    ) -> Result<(), CatalogueIngestionRepositoryError>;

    async fn upsert_route_collections(
        &self,
        records: &[RouteCollectionIngestion],
    ) -> Result<(), CatalogueIngestionRepositoryError>;

    async fn upsert_route_summaries(
        &self,
        records: &[RouteSummaryIngestion],
    ) -> Result<(), CatalogueIngestionRepositoryError>;

    async fn upsert_trending_highlights(
        &self,
        records: &[TrendingRouteHighlightIngestion],
    ) -> Result<(), CatalogueIngestionRepositoryError>;

    async fn upsert_community_picks(
        &self,
        records: &[CommunityPickIngestion],
    ) -> Result<(), CatalogueIngestionRepositoryError>;
}

/// Fixture implementation for tests that do not exercise ingestion writes.
#[derive(Debug, Default, Clone, Copy)]
pub struct FixtureCatalogueIngestionRepository;

#[async_trait]
impl CatalogueIngestionRepository for FixtureCatalogueIngestionRepository {
    async fn upsert_route_categories(
        &self,
        _records: &[RouteCategoryIngestion],
    ) -> Result<(), CatalogueIngestionRepositoryError> {
        Ok(())
    }

    async fn upsert_themes(
        &self,
        _records: &[ThemeIngestion],
    ) -> Result<(), CatalogueIngestionRepositoryError> {
        Ok(())
    }

    async fn upsert_route_collections(
        &self,
        _records: &[RouteCollectionIngestion],
    ) -> Result<(), CatalogueIngestionRepositoryError> {
        Ok(())
    }

    async fn upsert_route_summaries(
        &self,
        _records: &[RouteSummaryIngestion],
    ) -> Result<(), CatalogueIngestionRepositoryError> {
        Ok(())
    }

    async fn upsert_trending_highlights(
        &self,
        _records: &[TrendingRouteHighlightIngestion],
    ) -> Result<(), CatalogueIngestionRepositoryError> {
        Ok(())
    }

    async fn upsert_community_picks(
        &self,
        _records: &[CommunityPickIngestion],
    ) -> Result<(), CatalogueIngestionRepositoryError> {
        Ok(())
    }
}
