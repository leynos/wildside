//! Test helpers for Redis cache adapter tests.
//!
//! This module provides in-memory fakes and shared test utilities for unit
//! testing the Redis route cache adapter without requiring a live Redis server.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::domain::ports::{RouteCache, RouteCacheError, RouteCacheKey};
use crate::outbound::cache::redis_route_cache::ConnectionProvider;

/// Cached value with optional TTL in seconds.
type CachedValue = (Vec<u8>, Option<u64>);

/// Simple in-memory store used as a [`ConnectionProvider`] double.
#[derive(Debug, Clone)]
pub struct FakeProvider {
    store: Arc<Mutex<HashMap<String, CachedValue>>>,
}

impl FakeProvider {
    /// Create an empty fake provider with no pre-seeded data.
    pub fn empty() -> Self {
        Self {
            store: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a fake provider pre-seeded with a key-value pair.
    pub fn seeded(key: &str, bytes: Vec<u8>) -> Self {
        let mut map = HashMap::new();
        map.insert(key.to_owned(), (bytes, None));
        Self {
            store: Arc::new(Mutex::new(map)),
        }
    }

    /// Retrieve the recorded TTL for a given key, if any.
    ///
    /// Returns `None` if the key does not exist or was stored without TTL.
    pub fn ttl_for(&self, key: &str) -> Option<u64> {
        self.store
            .lock()
            .ok()
            .and_then(|store| store.get(key).and_then(|(_, ttl)| *ttl))
    }

    fn insert_entry(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl: Option<u64>,
    ) -> Result<(), RouteCacheError> {
        let mut store = self.store.lock().map_err(|_| RouteCacheError::Backend {
            message: "fake store lock poisoned".to_owned(),
        })?;
        store.insert(key.to_owned(), (value, ttl));
        Ok(())
    }
}

#[async_trait]
impl ConnectionProvider for FakeProvider {
    async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>, RouteCacheError> {
        let store = self.store.lock().map_err(|_| RouteCacheError::Backend {
            message: "fake store lock poisoned".to_owned(),
        })?;
        Ok(store.get(key).map(|(bytes, _)| bytes.clone()))
    }

    async fn set_bytes(&self, key: &str, value: Vec<u8>) -> Result<(), RouteCacheError> {
        self.insert_entry(key, value, None)
    }

    async fn set_bytes_ex(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl_seconds: u64,
    ) -> Result<(), RouteCacheError> {
        self.insert_entry(key, value, Some(ttl_seconds))
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

/// Assert that a cache correctly round-trips a default test plan.
///
/// Delegates to `assert_put_get_round_trips_with_plan` with `P::default()`.
pub async fn assert_put_get_round_trips<P>(
    cache: &impl RouteCache<Plan = P>,
) -> Result<(), RouteCacheError>
where
    P: Clone + PartialEq + std::fmt::Debug + Default,
{
    assert_put_get_round_trips_with_plan(cache, &P::default()).await
}

/// Assert that a cache correctly round-trips a specific plan instance.
pub async fn assert_put_get_round_trips_with_plan<P>(
    cache: &impl RouteCache<Plan = P>,
    plan: &P,
) -> Result<(), RouteCacheError>
where
    P: Clone + PartialEq + std::fmt::Debug,
{
    let key = RouteCacheKey::new("route:round-trip").map_err(|error| RouteCacheError::Backend {
        message: error.to_string(),
    })?;
    cache.put(&key, plan).await?;
    let loaded = cache.get(&key).await?;
    assert_eq!(loaded, Some(plan.clone()));
    Ok(())
}
