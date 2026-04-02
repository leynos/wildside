//! Test helpers for Redis cache adapter tests.
//!
//! This module provides in-memory fakes and shared test utilities for unit
//! testing the Redis route cache adapter without requiring a live Redis server.

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

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
        let store = self.store.lock().map_err(|_| RouteCacheError::Backend {
            message: "fake store lock poisoned".to_owned(),
        })?;
        Ok(store.get(key).cloned())
    }

    async fn set_bytes(&self, key: &str, value: Vec<u8>) -> Result<(), RouteCacheError> {
        let mut store = self.store.lock().map_err(|_| RouteCacheError::Backend {
            message: "fake store lock poisoned".to_owned(),
        })?;
        store.insert(key.to_owned(), value);
        Ok(())
    }
}

/// Test plan type used for cache adapter tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TestPlan {
    /// Request identifier for the test plan.
    pub request_id: String,
    /// Checksum value for the test plan.
    pub checksum: u64,
}

impl TestPlan {
    /// Create a new test plan with the given request ID and checksum.
    pub fn new(request_id: &str, checksum: u64) -> Self {
        Self {
            request_id: request_id.to_owned(),
            checksum,
        }
    }
}

/// Assert that a cache correctly round-trips a test plan.
///
/// # Type Parameters
///
/// - `P`: The plan type, must be serializable and comparable.
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
    P: Clone + PartialEq + std::fmt::Debug + Default,
{
    let key = RouteCacheKey::new("route:round-trip").unwrap_or_else(|e| {
        panic!("valid key: {e}");
    });
    let plan = P::default();
    cache
        .put(&key, &plan)
        .await
        .unwrap_or_else(|e| panic!("put succeeds: {e}"));
    let loaded = cache
        .get(&key)
        .await
        .unwrap_or_else(|e| panic!("get succeeds: {e}"));
    assert_eq!(loaded, Some(plan));
}

/// Assert that a cache correctly round-trips a specific plan instance.
///
/// This variant accepts a concrete plan value for use in parameterized tests.
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
    let key = RouteCacheKey::new("route:round-trip").unwrap_or_else(|e| {
        panic!("valid key: {e}");
    });
    cache
        .put(&key, plan)
        .await
        .unwrap_or_else(|e| panic!("put succeeds: {e}"));
    let loaded = cache
        .get(&key)
        .await
        .unwrap_or_else(|e| panic!("get succeeds: {e}"));
    assert_eq!(loaded, Some(plan.clone()));
}
