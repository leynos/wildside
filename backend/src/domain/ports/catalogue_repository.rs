//! Read-side port for catalogue snapshot retrieval.
//!
//! This port provides a domain-owned snapshot of the explore catalogue,
//! keeping persistence details behind the hexagonal boundary.  Inbound
//! adapters (for example HTTP endpoints in 3.2.3) consume the snapshot
//! type without coupling to Diesel or any specific data store.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::domain::{
    CommunityPick, RouteCategory, RouteCollection, RouteSummary, Theme, TrendingRouteHighlight,
};

use crate::domain::Error;

use super::define_port_error;

define_port_error! {
    /// Errors raised when reading catalogue snapshots.
    pub enum CatalogueRepositoryError {
        /// Repository connection could not be established.
        Connection { message: String } =>
            "catalogue read connection failed: {message}",
        /// Query failed during execution or row conversion.
        Query { message: String } =>
            "catalogue read query failed: {message}",
    }
}

/// Cohesive snapshot of the explore catalogue for the PWA landing page.
///
/// The snapshot is assembled from multiple tables in a single port call so
/// that consumers receive a consistent view without managing individual
/// query sequencing.
#[derive(Debug, Clone)]
pub struct ExploreCatalogueSnapshot {
    pub generated_at: DateTime<Utc>,
    pub categories: Vec<RouteCategory>,
    pub routes: Vec<RouteSummary>,
    pub themes: Vec<Theme>,
    pub collections: Vec<RouteCollection>,
    pub trending: Vec<TrendingRouteHighlight>,
    pub community_pick: Option<CommunityPick>,
}

impl ExploreCatalogueSnapshot {
    /// Construct an empty snapshot for fixture and fallback paths.
    pub fn empty() -> Self {
        Self {
            generated_at: DateTime::<Utc>::default(),
            categories: Vec::new(),
            routes: Vec::new(),
            themes: Vec::new(),
            collections: Vec::new(),
            trending: Vec::new(),
            community_pick: None,
        }
    }
}

/// Port for reading explore catalogue snapshots.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait CatalogueRepository: Send + Sync {
    /// Assemble and return the current explore catalogue snapshot.
    ///
    /// All entity collections are deterministically ordered (typically by
    /// slug).  An empty catalogue yields empty vectors and `None` for the
    /// community pick rather than an error.
    ///
    /// # Examples
    ///
    /// ```
    /// # use backend::domain::ports::{CatalogueRepository, FixtureCatalogueRepository};
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// let repo = FixtureCatalogueRepository;
    /// let snapshot = repo.explore_snapshot().await.unwrap();
    /// assert!(snapshot.categories.is_empty());
    /// assert!(snapshot.community_pick.is_none());
    /// # });
    /// ```
    async fn explore_snapshot(&self) -> Result<ExploreCatalogueSnapshot, CatalogueRepositoryError>;
}

/// Fixture implementation for tests that do not exercise catalogue reads.
#[derive(Debug, Default, Clone, Copy)]
pub struct FixtureCatalogueRepository;

#[async_trait]
impl CatalogueRepository for FixtureCatalogueRepository {
    async fn explore_snapshot(&self) -> Result<ExploreCatalogueSnapshot, CatalogueRepositoryError> {
        Ok(ExploreCatalogueSnapshot::empty())
    }
}

impl From<CatalogueRepositoryError> for Error {
    fn from(err: CatalogueRepositoryError) -> Self {
        match err {
            CatalogueRepositoryError::Connection { message } => Error::service_unavailable(message),
            CatalogueRepositoryError::Query { message } => Error::internal(message),
        }
    }
}
