//! In-memory doubles exercising the domain port contracts.
use super::*;
use actix_rt::System;
use async_trait::async_trait;
use std::collections::HashMap;
use std::error::Error as StdError;
use std::sync::Mutex;

type TestResult<T = ()> = Result<T, Box<dyn StdError>>;

#[derive(Clone, Debug, PartialEq, Eq)]
struct StubPlan {
    request_id: String,
    checksum: u64,
}

impl StubPlan {
    fn new(id: impl Into<String>, checksum: u64) -> Self {
        Self {
            request_id: id.into(),
            checksum,
        }
    }

    fn request_id(&self) -> &str {
        self.request_id.as_str()
    }
}

#[derive(Default)]
struct InMemoryRouteRepository {
    store: Mutex<HashMap<String, StubPlan>>,
}

#[async_trait]
impl RouteRepository for InMemoryRouteRepository {
    type Plan = StubPlan;

    async fn save(&self, plan: &Self::Plan) -> Result<(), RoutePersistenceError> {
        let mut guard = self
            .store
            .lock()
            .map_err(|_| RoutePersistenceError::write("store poisoned"))?;
        guard.insert(plan.request_id().to_owned(), plan.clone());
        Ok(())
    }

    async fn find_by_request_id(
        &self,
        request_id: &str,
    ) -> Result<Option<Self::Plan>, RoutePersistenceError> {
        let guard = self
            .store
            .lock()
            .map_err(|_| RoutePersistenceError::write("store poisoned"))?;
        Ok(guard.get(request_id).cloned())
    }
}

#[test]
fn repository_round_trip() {
    let repo = InMemoryRouteRepository::default();
    let plan = StubPlan::new("req-1", 42);

    System::new().block_on(async move {
        repo.save(&plan).await.expect("save succeeds");
        let fetched = repo
            .find_by_request_id(plan.request_id())
            .await
            .expect("load succeeds");
        assert_eq!(fetched, Some(plan));
    });
}

#[derive(Default)]
struct InMemoryRouteCache {
    store: Mutex<HashMap<String, StubPlan>>,
}

#[async_trait]
impl RouteCache for InMemoryRouteCache {
    type Plan = StubPlan;

    async fn get(&self, key: &RouteCacheKey) -> Result<Option<Self::Plan>, RouteCacheError> {
        let guard = self
            .store
            .lock()
            .map_err(|_| RouteCacheError::backend("cache poisoned"))?;
        Ok(guard.get(key.as_str()).cloned())
    }

    async fn put(&self, key: &RouteCacheKey, plan: &Self::Plan) -> Result<(), RouteCacheError> {
        let mut guard = self
            .store
            .lock()
            .map_err(|_| RouteCacheError::backend("cache poisoned"))?;
        guard.insert(key.as_str().to_owned(), plan.clone());
        Ok(())
    }
}

fn route_key() -> TestResult<RouteCacheKey> {
    Ok(RouteCacheKey::new("cache:user:1")?)
}

#[test]
fn cache_stores_entries() -> TestResult {
    let cache = InMemoryRouteCache::default();
    let plan = StubPlan::new("req-2", 7);
    let route_key = route_key()?;

    System::new().block_on(async move {
        cache.put(&route_key, &plan).await.expect("put");
        let loaded = cache.get(&route_key).await.expect("get");
        assert_eq!(loaded, Some(plan));
    });
    Ok(())
}
