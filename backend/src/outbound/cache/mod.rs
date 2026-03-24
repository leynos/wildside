//! Redis-backed adapters implementing caching ports.
//!
//! This module keeps Redis-specific pooling and payload encoding details at the
//! driven edge of the hexagon.

mod redis_route_cache;

pub use redis_route_cache::RedisRouteCache;

// Export RedisPool only for test-support feature to avoid exposing bb8-redis in public API
#[cfg(feature = "test-support")]
pub use redis_route_cache::RedisPool;
