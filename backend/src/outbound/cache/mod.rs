//! Redis-backed adapters implementing caching ports.
//!
//! This module keeps Redis-specific pooling and payload encoding details at the
//! driven edge of the hexagon.

mod redis_route_cache;

pub use redis_route_cache::RedisRouteCache;

// Export GenericRedisRouteCache for test-support to enable testing with FakeProvider
#[cfg(any(test, feature = "test-support"))]
pub use redis_route_cache::GenericRedisRouteCache;

// Export RedisPool only for test-support feature to avoid exposing bb8-redis in public API
#[cfg(feature = "test-support")]
pub use redis_route_cache::RedisPool;

// Test helpers available in test builds and via test-support feature
#[cfg(any(test, feature = "test-support"))]
pub mod test_helpers;

// Internal tests are only available in test builds
#[cfg(test)]
mod tests;
