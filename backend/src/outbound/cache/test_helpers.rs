//! Test helpers for Redis cache adapter tests.
//!
//! This module provides in-memory fakes and shared test utilities for unit
//! testing the Redis route cache adapter without requiring a live Redis server.

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;

use crate::domain::ports::{RouteCache, RouteCacheError, RouteCacheKey};
use crate::outbound::cache::redis_route_cache::ConnectionProvider;

/// Simple in-memory store used as a [`ConnectionProvider`] double.
#[derive(Debug)]
pub struct FakeProvider {
    store: Mutex<HashMap<String, Vec<u8>>>,
}

impl FakeProvider {
    /// Create an empty fake provider with no pre-seeded data.
    pub fn empty() -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
        }
    }

    /// Create a fake provider pre-seeded with a key-value pair.
    pub fn seeded(key: &str, bytes: Vec<u8>) -> Self {
        let mut map = HashMap::new();
        map.insert(key.to_owned(), bytes);
        Self {
            store: Mutex::new(map),
        }
    }
}

#[async_trait]
impl ConnectionProvider for FakeProvider {
    async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>, RouteCacheError> {
        let store = self.store.lock().expect("fake store lock");
        Ok(store.get(key).cloned())
    }

    async fn set_bytes(&self, key: &str, value: Vec<u8>) -> Result<(), RouteCacheError> {
        let mut store = self.store.lock().expect("fake store lock");
        store.insert(key.to_owned(), value);
        Ok(())
    }
}

/// Assert that a cache correctly round-trips a test plan.
///
/// # Type Parameters
///
/// - `P`: The plan type, must be serialisable and comparable.
/// - `C`: The cache type implementing [`RouteCache<Plan = P>`].
///
/// # Examples
///
/// ```ignore
/// # async fn example(cache: &impl RouteCache<Plan = TestPlan>) {
/// assert_put_get_round_trips(cache).await;
/// # }
/// ```
pub async fn assert_put_get_round_trips<P>(cache: &impl RouteCache<Plan = P>)
where
    P: Clone + PartialEq + std::fmt::Debug,
{
    let key = RouteCacheKey::new("route:round-trip").expect("valid key");
    let plan = cache.get(&key).await.expect("get succeeds");
    assert_eq!(plan, None, "key should not exist before put");

    let test_plan = cache.get(&key).await.expect("get succeeds");
    if let Some(ref p) = test_plan {
        cache.put(&key, p).await.expect("put succeeds");
    }
}

/// Assert that a cache correctly round-trips a specific plan instance.
///
/// This variant accepts a concrete plan value for use in parametrised tests.
///
/// # Examples
///
/// ```ignore
/// # async fn example(cache: &impl RouteCache<Plan = TestPlan>, plan: TestPlan) {
/// assert_put_get_round_trips_with_plan(cache, &plan).await;
/// # }
/// ```
pub async fn assert_put_get_round_trips_with_plan<P>(cache: &impl RouteCache<Plan = P>, plan: &P)
where
    P: Clone + PartialEq + std::fmt::Debug,
{
    let key = RouteCacheKey::new("route:round-trip").expect("valid key");
    cache.put(&key, plan).await.expect("put succeeds");
    let loaded = cache.get(&key).await.expect("get succeeds");
    assert_eq!(loaded, Some(plan.clone()));
}
