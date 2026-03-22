//! Redis-backed adapters implementing caching ports.
//!
//! This module keeps Redis-specific pooling and payload encoding details at the
//! driven edge of the hexagon.

mod redis_route_cache;

pub use redis_route_cache::{RedisPool, RedisRouteCache};
