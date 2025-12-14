//! Placeholder for future Redis cache adapter.
//!
//! This module provides a stub implementation of the `RouteCache` port that
//! always returns cache misses. It serves as a structural placeholder until
//! the Redis-backed implementation is completed.
//!
//! # Future Implementation
//!
//! The full Redis implementation will:
//! - Use `bb8-redis` for connection pooling
//! - Serialize plans with `serde_json` or MessagePack
//! - Apply TTL with jitter to prevent thundering herd on expiry
//! - Use namespaced keys (`route:v1:<sha256>`) for version-safe invalidation
//!
//! # Roadmap
//!
//! See `docs/backend-roadmap.md` for the Redis cache implementation tasks.

use std::marker::PhantomData;

use async_trait::async_trait;

use crate::domain::ports::{RouteCache, RouteCacheError, RouteCacheKey};

/// Stub cache implementation that always returns cache misses.
///
/// This placeholder implements the `RouteCache` port with no-op behaviour,
/// allowing the application to compile and run without a Redis backend.
/// All `get` operations return `None`; all `put` operations succeed silently.
///
/// The generic parameter `P` allows this stub to be used with any plan type,
/// enabling transparent substitution when the real Redis adapter is introduced.
///
/// # Why Default is manually implemented
///
/// Deriving `Default` on a generic struct adds a `P: Default` bound, but
/// `PhantomData<P>` implements `Default` unconditionally. The manual impl
/// avoids requiring plan types to implement `Default`.
#[derive(Debug, Clone)]
pub struct StubRouteCache<P> {
    _marker: PhantomData<P>,
}

impl<P> Default for StubRouteCache<P> {
    fn default() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<P> StubRouteCache<P> {
    /// Create a new stub cache instance.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl<P: Send + Sync> RouteCache for StubRouteCache<P> {
    type Plan = P;

    async fn get(&self, _key: &RouteCacheKey) -> Result<Option<Self::Plan>, RouteCacheError> {
        // Stub always misses; real implementation will query Redis.
        Ok(None)
    }

    async fn put(&self, _key: &RouteCacheKey, _plan: &Self::Plan) -> Result<(), RouteCacheError> {
        // Stub discards writes; real implementation will SET with TTL.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test plan type for unit tests.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestPlan;

    #[tokio::test]
    async fn stub_cache_always_misses() {
        let cache: StubRouteCache<TestPlan> = StubRouteCache::new();
        let key = RouteCacheKey::new("test:key:1").expect("valid key");

        let result = cache.get(&key).await.expect("get succeeds");
        assert!(result.is_none(), "stub cache should always miss");
    }

    #[tokio::test]
    async fn stub_cache_put_succeeds() {
        let cache: StubRouteCache<TestPlan> = StubRouteCache::new();
        let key = RouteCacheKey::new("test:key:2").expect("valid key");
        let plan = TestPlan;

        let result = cache.put(&key, &plan).await;
        assert!(result.is_ok(), "stub cache put should succeed");
    }
}
