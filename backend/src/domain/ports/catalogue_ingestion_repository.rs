//! Port abstraction for catalogue ingestion writes.
//!
//! This port keeps catalogue materialization behind the domain boundary so
//! ingestion jobs can evolve without leaking persistence details.

use async_trait::async_trait;

use crate::domain::{
    CommunityPick, RouteCategory, RouteCollection, RouteSummary, Theme, TrendingRouteHighlight,
};

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

/// Port for writing catalogue snapshots.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait CatalogueIngestionRepository: Send + Sync {
    /// Persist route category snapshots keyed by category identifier.
    ///
    /// Existing rows are updated with incoming payload values.
    async fn upsert_route_categories(
        &self,
        records: &[RouteCategory],
    ) -> Result<(), CatalogueIngestionRepositoryError>;

    /// Persist theme snapshots keyed by theme identifier.
    ///
    /// Existing rows are updated with incoming payload values.
    async fn upsert_themes(
        &self,
        records: &[Theme],
    ) -> Result<(), CatalogueIngestionRepositoryError>;

    /// Persist route collection snapshots keyed by collection identifier.
    ///
    /// Existing rows are updated with incoming payload values.
    async fn upsert_route_collections(
        &self,
        records: &[RouteCollection],
    ) -> Result<(), CatalogueIngestionRepositoryError>;

    /// Persist route summary snapshots keyed by summary identifier.
    ///
    /// Existing rows are updated with incoming payload values.
    async fn upsert_route_summaries(
        &self,
        records: &[RouteSummary],
    ) -> Result<(), CatalogueIngestionRepositoryError>;

    /// Persist trending highlight overlays keyed by highlight identifier.
    ///
    /// Existing rows are updated with incoming payload values.
    async fn upsert_trending_highlights(
        &self,
        records: &[TrendingRouteHighlight],
    ) -> Result<(), CatalogueIngestionRepositoryError>;

    /// Persist community pick snapshots keyed by pick identifier.
    ///
    /// Existing rows are updated with incoming payload values.
    async fn upsert_community_picks(
        &self,
        records: &[CommunityPick],
    ) -> Result<(), CatalogueIngestionRepositoryError>;
}

/// Fixture implementation for tests that do not exercise ingestion writes.
#[derive(Debug, Default, Clone, Copy)]
pub struct FixtureCatalogueIngestionRepository;

#[async_trait]
impl CatalogueIngestionRepository for FixtureCatalogueIngestionRepository {
    async fn upsert_route_categories(
        &self,
        _records: &[RouteCategory],
    ) -> Result<(), CatalogueIngestionRepositoryError> {
        Ok(())
    }

    async fn upsert_themes(
        &self,
        _records: &[Theme],
    ) -> Result<(), CatalogueIngestionRepositoryError> {
        Ok(())
    }

    async fn upsert_route_collections(
        &self,
        _records: &[RouteCollection],
    ) -> Result<(), CatalogueIngestionRepositoryError> {
        Ok(())
    }

    async fn upsert_route_summaries(
        &self,
        _records: &[RouteSummary],
    ) -> Result<(), CatalogueIngestionRepositoryError> {
        Ok(())
    }

    async fn upsert_trending_highlights(
        &self,
        _records: &[TrendingRouteHighlight],
    ) -> Result<(), CatalogueIngestionRepositoryError> {
        Ok(())
    }

    async fn upsert_community_picks(
        &self,
        _records: &[CommunityPick],
    ) -> Result<(), CatalogueIngestionRepositoryError> {
        Ok(())
    }
}
