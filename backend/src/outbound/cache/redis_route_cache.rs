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

/// Redis implementation of the [`RouteCache`] port.
///
/// The adapter stores JSON-encoded plan payloads as raw bytes so the domain
/// contract stays generic over plan shape while Redis remains an infrastructure
/// concern.
#[derive(Debug, Clone)]
pub struct RedisRouteCache<P> {
    pool: RedisPool,
    _plan: PhantomData<fn() -> P>,
}

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
    pub fn new(pool: RedisPool) -> Self {
        Self {
            pool,
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

#[async_trait]
impl<P> RouteCache for RedisRouteCache<P>
where
    P: Serialize + DeserializeOwned + Send + Sync,
{
    type Plan = P;

    async fn get(&self, key: &RouteCacheKey) -> Result<Option<Self::Plan>, RouteCacheError> {
        let mut connection = self.pool.get().await.map_err(map_pool_error)?;
        let payload = connection
            .get::<_, Option<Vec<u8>>>(key.as_str())
            .await
            .map_err(map_redis_error)?;

        payload
            .map(|bytes| serde_json::from_slice(&bytes).map_err(map_serialization_error))
            .transpose()
    }

    async fn put(&self, key: &RouteCacheKey, plan: &Self::Plan) -> Result<(), RouteCacheError> {
        let payload = serde_json::to_vec(plan).map_err(map_serialization_error)?;
        let mut connection = self.pool.get().await.map_err(map_pool_error)?;
        connection
            .set::<_, _, ()>(key.as_str(), payload)
            .await
            .map_err(map_redis_error)
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
    //! Tests requiring a real `redis-server` binary are marked with `#[ignore]`
    //! and can be run explicitly via:
    //! ```sh
    //! cargo test -- --ignored
    //! ```

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

    #[tokio::test]
    #[ignore = "requires redis-server binary; opt-in via RUN_REDIS_TESTS=1"]
    async fn get_returns_none_for_missing_key() {
        let server = TestRedisServer::start().await;
        let cache = RedisRouteCache::<TestPlan>::new(server.pool().await.expect("redis pool"));
        let key = RouteCacheKey::new("route:missing").expect("valid key");

        let result = cache.get(&key).await.expect("missing-key lookup succeeds");

        assert_eq!(result, None);
    }

    #[tokio::test]
    #[ignore = "requires redis-server binary; opt-in via RUN_REDIS_TESTS=1"]
    async fn put_followed_by_get_round_trips_the_typed_plan() {
        let server = TestRedisServer::start().await;
        let cache = RedisRouteCache::<TestPlan>::new(server.pool().await.expect("redis pool"));
        let key = RouteCacheKey::new("route:round-trip").expect("valid key");
        let plan = TestPlan::new("req-1", 42);

        cache.put(&key, &plan).await.expect("put succeeds");

        let loaded = cache.get(&key).await.expect("get succeeds");

        assert_eq!(loaded, Some(plan));
    }

    #[tokio::test]
    #[ignore = "requires redis-server binary; opt-in via RUN_REDIS_TESTS=1"]
    async fn corrupted_cached_bytes_map_to_serialization_errors() {
        let server = TestRedisServer::start().await;
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
        let unreachable_url = unused_redis_url().await;
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
