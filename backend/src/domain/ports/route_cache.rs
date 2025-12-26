//! Port interface for caching computed route plans.
use async_trait::async_trait;

use super::{RouteCacheKey, define_port_error};

define_port_error! {
    /// Errors surfaced by the caching adapter.
    pub enum RouteCacheError {
        /// Cache backend is unavailable or timing out.
        Backend { message: String } => "route cache backend failure: {message}",
        /// Serialisation or deserialisation of cached content failed.
        Serialization { message: String } => "route cache serialisation failed: {message}",
    }
}

#[async_trait]
pub trait RouteCache: Send + Sync {
    /// Domain-specific plan representation shared with the repository.
    type Plan: Send + Sync;

    /// Read a cached plan for the given key.
    async fn get(&self, key: &RouteCacheKey) -> Result<Option<Self::Plan>, RouteCacheError>;

    /// Store a plan in the cache using the supplied key.
    async fn put(&self, key: &RouteCacheKey, plan: &Self::Plan) -> Result<(), RouteCacheError>;
}
