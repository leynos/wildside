//! Domain ports defining the edges of the hexagon.
//!
//! Ports describe how the domain expects to interact with driven adapters
//! (databases, caches, queues, metrics exporters). Each trait exposes
//! strongly typed errors so adapters map their failures into predictable
//! variants instead of returning `anyhow::Result`.

use std::fmt;

use async_trait::async_trait;
use thiserror::Error;

use super::{User, UserId};

/// Cache key used to store and retrieve canonicalised route plans.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RouteCacheKey(String);

impl RouteCacheKey {
    /// Construct a cache key after validating that it is non-empty and
    /// trimmed.
    ///
    /// # Examples
    /// ```
    /// use backend::domain::ports::RouteCacheKey;
    ///
    /// let key = RouteCacheKey::new("user:123:route:abc").expect("valid key");
    /// assert_eq!(key.as_str(), "user:123:route:abc");
    /// ```
    pub fn new(value: impl Into<String>) -> Result<Self, RouteCacheKeyValidationError> {
        let raw = value.into();
        if raw.trim().is_empty() {
            return Err(RouteCacheKeyValidationError::Empty);
        }
        if raw.trim() != raw {
            return Err(RouteCacheKeyValidationError::ContainsWhitespace);
        }
        Ok(Self(raw))
    }

    /// Borrow the underlying key as a string slice.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for RouteCacheKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for RouteCacheKey {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Validation errors returned when constructing [`RouteCacheKey`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RouteCacheKeyValidationError {
    /// Key is empty after trimming whitespace.
    #[error("route cache key must not be empty")]
    Empty,
    /// Key contains leading or trailing whitespace.
    #[error("route cache key must not contain surrounding whitespace")]
    ContainsWhitespace,
}

/// Errors surfaced by the persistence adapter when handling routes.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RoutePersistenceError {
    /// Database connectivity or transaction failures.
    #[error("route persistence connection failed: {message}")]
    Connection { message: String },
    /// Duplicate request identifiers detected.
    #[error("route conflict detected for request {request_id}")]
    Conflict { request_id: String },
    /// Catch-all for write failures that bubble up from the adapter.
    #[error("route persistence failed: {message}")]
    Write { message: String },
}

impl RoutePersistenceError {
    /// Helper for connection related adapter errors.
    pub fn connection(message: impl Into<String>) -> Self {
        Self::Connection {
            message: message.into(),
        }
    }

    /// Helper for write failures.
    pub fn write(message: impl Into<String>) -> Self {
        Self::Write {
            message: message.into(),
        }
    }

    /// Helper for idempotency conflicts.
    pub fn conflict(request_id: impl Into<String>) -> Self {
        Self::Conflict {
            request_id: request_id.into(),
        }
    }
}

/// Errors surfaced by the caching adapter.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RouteCacheError {
    /// Cache backend is unavailable or timing out.
    #[error("route cache backend failure: {message}")]
    Backend { message: String },
    /// Serialisation or deserialisation of cached content failed.
    #[error("route cache serialisation failed: {message}")]
    Serialization { message: String },
}

impl RouteCacheError {
    /// Helper for backend-level failures.
    pub fn backend(message: impl Into<String>) -> Self {
        Self::Backend {
            message: message.into(),
        }
    }

    /// Helper for serialisation problems.
    pub fn serialization(message: impl Into<String>) -> Self {
        Self::Serialization {
            message: message.into(),
        }
    }
}

/// Errors surfaced by the queue/dispatcher adapter.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum JobDispatchError {
    /// Queue infrastructure is unavailable.
    #[error("route queue is unavailable: {message}")]
    Unavailable { message: String },
    /// The job could not be acknowledged or persisted.
    #[error("route job was rejected: {message}")]
    Rejected { message: String },
}

impl JobDispatchError {
    /// Helper for queue outages.
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::Unavailable {
            message: message.into(),
        }
    }

    /// Helper for rejected jobs.
    pub fn rejected(message: impl Into<String>) -> Self {
        Self::Rejected {
            message: message.into(),
        }
    }
}

/// Errors exposed when recording metrics.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RouteMetricsError {
    /// Metric exporter rejected the write.
    #[error("route metrics exporter failed: {message}")]
    Export { message: String },
}

impl RouteMetricsError {
    /// Helper for exporter write failures.
    pub fn export(message: impl Into<String>) -> Self {
        Self::Export {
            message: message.into(),
        }
    }
}

/// Persistence errors raised by [`UserRepository`] adapters.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum UserPersistenceError {
    /// Repository connection could not be established.
    #[error("user repository connection failed: {message}")]
    Connection { message: String },
    /// Query or mutation failed during execution.
    #[error("user repository query failed: {message}")]
    Query { message: String },
}

impl UserPersistenceError {
    /// Helper for connection oriented failures.
    pub fn connection(message: impl Into<String>) -> Self {
        Self::Connection {
            message: message.into(),
        }
    }

    /// Helper for query failures.
    pub fn query(message: impl Into<String>) -> Self {
        Self::Query {
            message: message.into(),
        }
    }
}

/// Persistence port for storing and retrieving route plans.
#[async_trait]
pub trait RouteRepository: Send + Sync {
    /// Domain-specific plan representation.
    type Plan: Send + Sync;

    /// Persist a route plan.
    ///
    /// # Examples
    /// ```no_run
    /// use async_trait::async_trait;
    /// use backend::domain::ports::{RoutePersistenceError, RouteRepository};
    ///
    /// #[derive(Clone)]
    /// struct MemoryRepo;
    ///
    /// #[async_trait]
    /// impl RouteRepository for MemoryRepo {
    ///     type Plan = String;
    ///
    ///     async fn save(&self, plan: &Self::Plan) -> Result<(), RoutePersistenceError> {
    ///         println!("persisting {plan}");
    ///         Ok(())
    ///     }
    ///
    ///     async fn find_by_request_id(
    ///         &self,
    ///         request_id: &str,
    ///     ) -> Result<Option<Self::Plan>, RoutePersistenceError> {
    ///         Ok(Some(format!("plan::{request_id}")))
    ///     }
    /// }
    /// ```
    async fn save(&self, plan: &Self::Plan) -> Result<(), RoutePersistenceError>;

    /// Fetch a plan by its request identifier.
    async fn find_by_request_id(
        &self,
        request_id: &str,
    ) -> Result<Option<Self::Plan>, RoutePersistenceError>;
}

/// Cache port allowing adapters to keep hot plans resident.
#[async_trait]
pub trait RouteCache: Send + Sync {
    /// Domain-specific plan representation shared with the repository.
    type Plan: Send + Sync;

    /// Read a cached plan for the given key.
    async fn get(&self, key: &RouteCacheKey) -> Result<Option<Self::Plan>, RouteCacheError>;

    /// Store a plan in the cache using the supplied key.
    async fn put(&self, key: &RouteCacheKey, plan: &Self::Plan) -> Result<(), RouteCacheError>;
}

/// Queue port for dispatching route generation work.
#[async_trait]
pub trait RouteQueue: Send + Sync {
    /// Domain-specific plan representation shared across adapters.
    type Plan: Send + Sync;

    /// Enqueue a plan for downstream processing.
    async fn enqueue(&self, plan: &Self::Plan) -> Result<(), JobDispatchError>;
}

/// Metrics port for recording cache effectiveness.
#[async_trait]
pub trait RouteMetrics: Send + Sync {
    /// Record a cache hit.
    async fn record_cache_hit(&self) -> Result<(), RouteMetricsError>;

    /// Record a cache miss.
    async fn record_cache_miss(&self) -> Result<(), RouteMetricsError>;
}

/// Persistence port for user aggregates.
#[async_trait]
pub trait UserRepository: Send + Sync {
    /// Insert or update a user record.
    async fn upsert(&self, user: &User) -> Result<(), UserPersistenceError>;

    /// Fetch a user by identifier.
    async fn find_by_id(&self, id: &UserId) -> Result<Option<User>, UserPersistenceError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_rt::System;
    use async_trait::async_trait;
    use rstest::{fixture, rstest};
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[rstest]
    #[case("")]
    #[case("   ")]
    fn cache_key_rejects_blank(#[case] value: &str) {
        let err = RouteCacheKey::new(value).expect_err("blank keys rejected");
        assert_eq!(err, RouteCacheKeyValidationError::Empty);
    }

    #[rstest]
    #[case(" leading")]
    #[case("trailing ")]
    fn cache_key_rejects_whitespace_padding(#[case] value: &str) {
        let err = RouteCacheKey::new(value).expect_err("padded key rejected");
        assert_eq!(err, RouteCacheKeyValidationError::ContainsWhitespace);
    }

    #[rstest]
    fn cache_key_accepts_clean_input() {
        let key = RouteCacheKey::new("route:user:1").expect("valid key");
        assert_eq!(key.as_str(), "route:user:1");
        assert_eq!(key.to_string(), "route:user:1");
    }

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
            let mut guard = self.store.lock().expect("store poisoned");
            guard.insert(plan.request_id().to_owned(), plan.clone());
            Ok(())
        }

        async fn find_by_request_id(
            &self,
            request_id: &str,
        ) -> Result<Option<Self::Plan>, RoutePersistenceError> {
            let guard = self.store.lock().expect("store poisoned");
            Ok(guard.get(request_id).cloned())
        }
    }

    #[rstest]
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
            let guard = self.store.lock().expect("cache poisoned");
            Ok(guard.get(key.as_str()).cloned())
        }

        async fn put(&self, key: &RouteCacheKey, plan: &Self::Plan) -> Result<(), RouteCacheError> {
            let mut guard = self.store.lock().expect("cache poisoned");
            guard.insert(key.as_str().to_owned(), plan.clone());
            Ok(())
        }
    }

    #[fixture]
    fn route_key() -> RouteCacheKey {
        RouteCacheKey::new("cache:user:1").expect("valid key")
    }

    #[rstest]
    fn cache_stores_entries(route_key: RouteCacheKey) {
        let cache = InMemoryRouteCache::default();
        let plan = StubPlan::new("req-2", 7);

        System::new().block_on(async move {
            cache.put(&route_key, &plan).await.expect("put");
            let loaded = cache.get(&route_key).await.expect("get");
            assert_eq!(loaded, Some(plan));
        });
    }
}
