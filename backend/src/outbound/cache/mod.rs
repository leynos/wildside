//! Redis-backed adapters implementing caching ports.
//!
//! This module keeps Redis-specific pooling and payload encoding details at the
//! driven edge of the hexagon.

mod redis_route_cache;

pub use redis_route_cache::{RedisRouteCache, jittered_ttl};

// Export RedisPool only for test-support feature to avoid exposing bb8-redis in public API
#[cfg(feature = "test-support")]
pub use redis_route_cache::RedisPool;

// Test helpers and internal tests are only available in test builds
#[cfg(test)]
pub mod test_helpers;

#[cfg(test)]
mod tests;
