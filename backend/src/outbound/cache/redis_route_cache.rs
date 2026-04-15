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
use rand::Rng;
use serde::{Serialize, de::DeserializeOwned};

use crate::domain::ports::{RouteCache, RouteCacheError, RouteCacheKey};

/// Shared pool type for Redis-backed adapters.
pub type RedisPool = Pool<RedisConnectionManager>;

/// Default base TTL for cache entries (24 hours in seconds).
pub const DEFAULT_BASE_TTL_SECS: u64 = 86_400;

/// Default jitter fraction (+/- 10% of base TTL).
pub const DEFAULT_JITTER_FRACTION: f64 = 0.10;

/// Compute a TTL in seconds with uniform random jitter.
///
/// Given a `base_ttl` of 86 400 (24 hours) and a `jitter_fraction` of 0.10,
/// the result will be uniformly distributed in the range [77 760, 95 040].
///
/// The jitter is applied by computing a delta range and adding a random offset:
/// - `delta = base_ttl * jitter_fraction`
/// - Random offset in `[0, 2 * delta]`
/// - Returns `(base_ttl - delta + offset).max(1)`
///
/// The `jitter_fraction` parameter is clamped to the range [0.0, 1.0] to ensure
/// sane results. Values outside this range will be automatically clamped.
///
/// # Examples
///
/// ```
/// use rand::rngs::StdRng;
/// use rand::SeedableRng;
///
/// # fn main() {
/// let mut rng = StdRng::seed_from_u64(42);
/// // jittered_ttl is crate-private, this example is for documentation only
/// # }
/// ```
pub(crate) fn jittered_ttl(base_ttl: u64, jitter_fraction: f64, rng: &mut impl Rng) -> u64 {
    let clamped_jitter = jitter_fraction.clamp(0.0, 1.0);
    let delta = (base_ttl as f64 * clamped_jitter) as u64;
    let max_offset = delta.saturating_mul(2);
    let offset = rng.gen_range(0..=max_offset);
    (base_ttl.saturating_sub(delta).saturating_add(offset)).max(1)
}

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

    /// Write raw bytes for `key` with optional TTL.
    ///
    /// If `ttl_seconds` is `Some(n)`, the entry expires after `n` seconds.
    /// If `ttl_seconds` is `None`, the entry persists without expiry.
    /// Implementations that do not support expiry may ignore the parameter.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # async fn example(provider: &dyn ConnectionProvider) -> Result<(), RouteCacheError> {
    /// // Store with 1-hour TTL
    /// provider.set_bytes_with_ttl("k", vec![1, 2, 3], Some(3600)).await?;
    /// let got = provider.get_bytes("k").await?;
    /// assert_eq!(got, Some(vec![1, 2, 3]));
    /// # Ok(())
    /// # }
    /// ```
    async fn set_bytes_with_ttl(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl_seconds: Option<u64>,
    ) -> Result<(), RouteCacheError>;

    /// Write raw bytes for `key` without expiry.
    ///
    /// This is a convenience wrapper around `set_bytes_with_ttl(key, value, None)`.
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
    async fn set_bytes(&self, key: &str, value: Vec<u8>) -> Result<(), RouteCacheError> {
        self.set_bytes_with_ttl(key, value, None).await
    }
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

    async fn set_bytes_with_ttl(
        &self,
        key: &str,
        value: Vec<u8>,
        ttl_seconds: Option<u64>,
    ) -> Result<(), RouteCacheError> {
        let mut connection = self.pool.get().await.map_err(map_pool_error)?;
        match ttl_seconds {
            Some(ttl) => connection
                .set_ex::<_, _, ()>(key, value, ttl)
                .await
                .map_err(map_redis_error),
            None => connection
                .set::<_, _, ()>(key, value)
                .await
                .map_err(map_redis_error),
        }
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
    base_ttl: u64,
    jitter_fraction: f64,
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
    fn from_pool(pool: RedisPool) -> Self {
        Self {
            provider: RedisPoolProvider { pool },
            base_ttl: DEFAULT_BASE_TTL_SECS,
            jitter_fraction: DEFAULT_JITTER_FRACTION,
            _plan: PhantomData,
        }
    }

    /// Creates a new cache instance from an existing pool.
    ///
    /// Available as public API when the `test-support` feature is enabled,
    /// otherwise crate-private to keep bb8-redis internals from leaking.
    #[cfg(feature = "test-support")]
    pub fn new(pool: RedisPool) -> Self {
        Self::from_pool(pool)
    }

    #[cfg(not(feature = "test-support"))]
    pub(crate) fn new(pool: RedisPool) -> Self {
        Self::from_pool(pool)
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
            base_ttl: DEFAULT_BASE_TTL_SECS,
            jitter_fraction: DEFAULT_JITTER_FRACTION,
            _plan: PhantomData,
        }
    }

    /// Create a cache with a custom provider and TTL parameters.
    ///
    /// This constructor allows tests to control TTL behaviour by specifying
    /// custom base TTL and jitter fraction.
    #[cfg(any(test, feature = "test-support"))]
    pub fn with_provider_and_ttl(provider: C, base_ttl: u64, jitter_fraction: f64) -> Self {
        Self {
            provider,
            base_ttl,
            jitter_fraction,
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

        let ttl = {
            let mut rng = rand::thread_rng();
            jittered_ttl(self.base_ttl, self.jitter_fraction, &mut rng)
        };

        self.provider
            .set_bytes_with_ttl(key.as_str(), payload, Some(ttl))
            .await
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
