//! Redis-backed `RouteCache` adapter.

use std::marker::PhantomData;

use async_trait::async_trait;
use bb8_redis::{
    RedisConnectionManager,
    bb8::{Pool, RunError},
    redis::{AsyncCommands, RedisError},
};
use serde::{Serialize, de::DeserializeOwned};

use crate::domain::ports::{RouteCache, RouteCacheError, RouteCacheKey};

/// Shared pool type for Redis-backed adapters.
pub type RedisPool = Pool<RedisConnectionManager>;

/// Thin abstraction over a connection pool for testability.
///
/// Production code uses [`RedisPoolProvider`] which wraps a [`RedisPool`].
/// Unit tests supply a fake that returns canned byte payloads without
/// touching a real Redis server.
#[async_trait]
pub trait ConnectionProvider: Send + Sync {
    /// Read raw bytes for `key`, returning `None` on a cache miss.
    async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>, RouteCacheError>;

    /// Write raw bytes for `key`.
    async fn set_bytes(&self, key: &str, value: Vec<u8>) -> Result<(), RouteCacheError>;
}

/// [`ConnectionProvider`] backed by a real `bb8-redis` pool.
///
/// Fields are private — external code constructs a cache via
/// [`RedisRouteCache::connect`]. The [`RedisRouteCache::new`] constructor is
/// provided only for test/support builds (enabled via the `test-support`
/// feature) to allow injection of pre-configured pools.
#[derive(Debug, Clone)]
pub struct RedisPoolProvider {
    pool: RedisPool,
}

#[async_trait]
impl ConnectionProvider for RedisPoolProvider {
    async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>, RouteCacheError> {
        let mut connection = self.pool.get().await.map_err(map_pool_error)?;
        connection
            .get::<_, Option<Vec<u8>>>(key)
            .await
            .map_err(map_redis_error)
    }

    async fn set_bytes(&self, key: &str, value: Vec<u8>) -> Result<(), RouteCacheError> {
        let mut connection = self.pool.get().await.map_err(map_pool_error)?;
        connection
            .set::<_, _, ()>(key, value)
            .await
            .map_err(map_redis_error)
    }
}

/// Internal generic adapter parameterised over the connection provider.
///
/// Production code uses the [`RedisRouteCache`] type alias which fixes the
/// provider to `RedisPoolProvider`. Unit tests substitute a fake provider to
/// exercise JSON round-trip logic without a live Redis server.
#[derive(Debug, Clone)]
pub struct GenericRedisRouteCache<P, C> {
    provider: C,
    _plan: PhantomData<fn() -> P>,
}

/// Redis implementation of the [`RouteCache`] port.
///
/// The adapter stores JSON-encoded plan payloads as raw bytes so the domain
/// contract stays generic over plan shape while Redis remains an infrastructure
/// concern.
pub type RedisRouteCache<P> = GenericRedisRouteCache<P, RedisPoolProvider>;

impl<P> RedisRouteCache<P> {
    /// Create an adapter from an existing Redis connection pool.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use backend::outbound::cache::RedisRouteCache;
    /// use bb8_redis::{bb8::Pool, RedisConnectionManager};
    ///
    /// # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
    /// let manager = RedisConnectionManager::new("redis://127.0.0.1/")?;
    /// let pool = Pool::builder().build(manager).await?;
    /// let cache = RedisRouteCache::<serde_json::Value>::new(pool);
    /// # let _ = cache;
    /// # Ok(())
    /// # }
    /// ```
    /// Creates a new cache instance from an existing pool.
    ///
    /// Available as public API when the `test-support` feature is enabled,
    /// otherwise crate-private to keep bb8-redis internals from leaking.
    #[cfg(feature = "test-support")]
    pub fn new(pool: RedisPool) -> Self {
        Self {
            provider: RedisPoolProvider { pool },
            _plan: PhantomData,
        }
    }

    #[cfg(not(feature = "test-support"))]
    pub(crate) fn new(pool: RedisPool) -> Self {
        Self {
            provider: RedisPoolProvider { pool },
            _plan: PhantomData,
        }
    }

    /// Build a Redis-backed cache from a Redis connection string.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use backend::outbound::cache::RedisRouteCache;
    ///
    /// # async fn demo() -> Result<(), backend::domain::ports::RouteCacheError> {
    /// let cache = RedisRouteCache::<serde_json::Value>::connect(
    ///     "redis://127.0.0.1/",
    /// )
    /// .await?;
    /// # let _ = cache;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect(redis_url: &str) -> Result<Self, RouteCacheError> {
        let manager = RedisConnectionManager::new(redis_url).map_err(map_redis_error)?;
        let pool = Pool::builder()
            .build(manager)
            .await
            .map_err(map_redis_error)?;
        Ok(Self::new(pool))
    }
}

#[cfg(test)]
impl<P, C> GenericRedisRouteCache<P, C> {
    fn with_provider(provider: C) -> Self {
        Self {
            provider,
            _plan: PhantomData,
        }
    }
}

#[async_trait]
impl<P, C> RouteCache for GenericRedisRouteCache<P, C>
where
    P: Serialize + DeserializeOwned + Send + Sync,
    C: ConnectionProvider,
{
    type Plan = P;

    async fn get(&self, key: &RouteCacheKey) -> Result<Option<Self::Plan>, RouteCacheError> {
        let payload = self.provider.get_bytes(key.as_str()).await?;

        payload
            .map(|bytes| serde_json::from_slice(&bytes).map_err(map_serialization_error))
            .transpose()
    }

    async fn put(&self, key: &RouteCacheKey, plan: &Self::Plan) -> Result<(), RouteCacheError> {
        let payload = serde_json::to_vec(plan).map_err(map_serialization_error)?;
        self.provider.set_bytes(key.as_str(), payload).await
    }
}

fn map_pool_error(error: RunError<RedisError>) -> RouteCacheError {
    RouteCacheError::Backend {
        message: error.to_string(),
    }
}

fn map_redis_error(error: RedisError) -> RouteCacheError {
    RouteCacheError::Backend {
        message: error.to_string(),
    }
}

fn map_serialization_error(error: serde_json::Error) -> RouteCacheError {
    RouteCacheError::Serialization {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    //! Focused adapter tests covering port-level semantics.
    //!
    //! Mock-based tests run unconditionally and cover JSON round-trip,
    //! cache-miss, and corrupt-payload semantics. Tests requiring a real
    //! `redis-server` binary are marked with `#[ignore]` and can be run
    //! via:
    //! ```sh
    //! cargo test -- --ignored
    //! ```

    use std::collections::HashMap;
    use std::sync::Mutex;

    use bb8_redis::redis::cmd;
    use rstest::rstest;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::test_support::redis::{RedisTestServer as TestRedisServer, unused_redis_url};

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct TestPlan {
        request_id: String,
        checksum: u64,
    }

    impl TestPlan {
        fn new(request_id: &str, checksum: u64) -> Self {
            Self {
                request_id: request_id.to_owned(),
                checksum,
            }
        }
    }

    // -- In-memory fake for ConnectionProvider ----------------------------------

    /// Simple in-memory store used as a [`ConnectionProvider`] double.
    #[derive(Debug)]
    struct FakeProvider {
        store: Mutex<HashMap<String, Vec<u8>>>,
    }

    impl FakeProvider {
        fn empty() -> Self {
            Self {
                store: Mutex::new(HashMap::new()),
            }
        }

        fn seeded(key: &str, bytes: Vec<u8>) -> Self {
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

    // -- Shared test helpers ------------------------------------------------------

    async fn assert_put_get_round_trips(cache: &impl RouteCache<Plan = TestPlan>) {
        let key = RouteCacheKey::new("route:round-trip").expect("valid key");
        let plan = TestPlan::new("req-1", 42);
        cache.put(&key, &plan).await.expect("put succeeds");
        let loaded = cache.get(&key).await.expect("get succeeds");
        assert_eq!(loaded, Some(plan));
    }

    // -- Mock-based tests (run unconditionally) ---------------------------------

    #[tokio::test]
    async fn mock_get_returns_none_for_missing_key() {
        let cache = GenericRedisRouteCache::<TestPlan, _>::with_provider(FakeProvider::empty());
        let key = RouteCacheKey::new("route:missing").expect("valid key");

        let result = cache.get(&key).await.expect("get should succeed");

        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn mock_put_then_get_round_trips_the_typed_plan() {
        let cache = GenericRedisRouteCache::<TestPlan, _>::with_provider(FakeProvider::empty());
        assert_put_get_round_trips(&cache).await;
    }

    #[tokio::test]
    async fn mock_corrupted_bytes_map_to_serialization_errors() {
        let cache = GenericRedisRouteCache::<TestPlan, _>::with_provider(FakeProvider::seeded(
            "route:corrupt",
            vec![0_u8, 159, 146, 150],
        ));
        let key = RouteCacheKey::new("route:corrupt").expect("valid key");

        let result = cache
            .get(&key)
            .await
            .expect_err("corrupt payload should fail");

        assert!(matches!(result, RouteCacheError::Serialization { .. }));
    }

    // -- Live Redis tests (require redis-server binary) -------------------------

    #[tokio::test]
    #[ignore = "requires redis-server binary; opt-in via RUN_REDIS_TESTS=1"]
    async fn get_returns_none_for_missing_key() {
        let server = TestRedisServer::start().await.expect("start redis-server");
        let cache = RedisRouteCache::<TestPlan>::new(server.pool().await.expect("redis pool"));
        let key = RouteCacheKey::new("route:missing").expect("valid key");

        let result = cache.get(&key).await.expect("missing-key lookup succeeds");

        assert_eq!(result, None);
    }

    #[tokio::test]
    #[ignore = "requires redis-server binary; opt-in via RUN_REDIS_TESTS=1"]
    async fn put_followed_by_get_round_trips_the_typed_plan() {
        let server = TestRedisServer::start().await.expect("start redis-server");
        let cache = RedisRouteCache::<TestPlan>::new(server.pool().await.expect("redis pool"));
        assert_put_get_round_trips(&cache).await;
    }

    #[tokio::test]
    #[ignore = "requires redis-server binary; opt-in via RUN_REDIS_TESTS=1"]
    async fn corrupted_cached_bytes_map_to_serialization_errors() {
        let server = TestRedisServer::start().await.expect("start redis-server");
        let pool = server.pool().await.expect("redis pool");
        let cache = RedisRouteCache::<TestPlan>::new(pool.clone());
        let key = RouteCacheKey::new("route:corrupt").expect("valid key");
        let mut connection = pool.get().await.expect("redis connection");

        cmd("SET")
            .arg(key.as_str())
            .arg(vec![0_u8, 159, 146, 150])
            .query_async::<()>(&mut *connection)
            .await
            .expect("seed corrupt bytes");

        let result = cache
            .get(&key)
            .await
            .expect_err("corrupt payload should fail");

        assert!(matches!(result, RouteCacheError::Serialization { .. }));
    }

    #[tokio::test]
    async fn command_failures_map_to_backend_errors() {
        let unreachable_url = unused_redis_url().await.expect("unused redis url");
        let manager = RedisConnectionManager::new(unreachable_url.as_str()).expect("redis manager");
        let pool = Pool::builder().max_size(1).build_unchecked(manager);
        let cache = RedisRouteCache::<TestPlan>::new(pool);
        let key = RouteCacheKey::new("route:backend").expect("valid key");

        let result = cache
            .get(&key)
            .await
            .expect_err("unreachable backend should fail");

        assert!(matches!(result, RouteCacheError::Backend { .. }));
    }

    #[rstest]
    #[case("not a redis url")]
    #[case("http://127.0.0.1:6379")]
    #[tokio::test]
    async fn connect_maps_invalid_connection_strings_to_backend_errors(#[case] redis_url: &str) {
        let result = RedisRouteCache::<TestPlan>::connect(redis_url)
            .await
            .expect_err("invalid redis url should fail");

        assert!(matches!(result, RouteCacheError::Backend { .. }));
    }
}
