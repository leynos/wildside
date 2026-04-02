//! Redis-backed [`RouteCache`] adapter for typed route plans.
//!
//! This module stores plan payloads as JSON-encoded bytes behind Redis,
//! validates keys through [`RouteCacheKey`], and maps Redis/pool failures plus
//! serialization failures into [`RouteCacheError`]. The adapter is built around
//! the [`ConnectionProvider`] abstraction so tests can substitute fakes or
//! mocks while production uses the [`RedisPool`] and [`RedisPoolProvider`]
//! backed implementation.

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
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # async fn example(provider: &dyn ConnectionProvider) -> Result<(), RouteCacheError> {
    /// let result = provider.get_bytes("missing_key").await?;
    /// assert_eq!(result, None);
    /// # Ok(())
    /// # }
    /// ```
    async fn get_bytes(&self, key: &str) -> Result<Option<Vec<u8>>, RouteCacheError>;

    /// Write raw bytes for `key`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # async fn example(provider: &dyn ConnectionProvider) -> Result<(), RouteCacheError> {
    /// provider.set_bytes("k", vec![1, 2, 3]).await?;
    /// let got = provider.get_bytes("k").await?;
    /// assert_eq!(got, Some(vec![1, 2, 3]));
    /// # Ok(())
    /// # }
    /// ```
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

/// Generic adapter parameterized over the connection provider.
///
/// Production code uses the [`RedisRouteCache`] type alias which fixes the
/// provider to `RedisPoolProvider`. Unit tests substitute a fake provider to
/// exercise JSON round-trip logic without a live Redis server.
///
/// Public because the [`RedisRouteCache`] type alias references this type;
/// prefer using the type alias for production code.
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

impl<P, C> GenericRedisRouteCache<P, C> {
    /// Create a cache with a custom connection provider.
    ///
    /// This constructor is crate-private and intended for test use only.
    /// Production code should use [`RedisRouteCache::connect`] or
    /// [`RedisRouteCache::new`].
    #[cfg(test)]
    pub(crate) fn with_provider(provider: C) -> Self {
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

fn map_backend_error<E: std::fmt::Display>(err: E) -> RouteCacheError {
    RouteCacheError::Backend {
        message: err.to_string(),
    }
}

fn map_pool_error(error: RunError<RedisError>) -> RouteCacheError {
    map_backend_error(error)
}

fn map_redis_error(error: RedisError) -> RouteCacheError {
    map_backend_error(error)
}

fn map_serialization_error(error: serde_json::Error) -> RouteCacheError {
    RouteCacheError::Serialization {
        message: error.to_string(),
    }
}
