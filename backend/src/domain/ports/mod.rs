//! Domain ports and supporting types for the hexagonal boundary.

mod macros;
pub(crate) use macros::define_port_error;

mod cache_key;
mod route_cache;
mod route_metrics;
mod route_queue;
mod route_repository;
mod user_repository;

pub use cache_key::{RouteCacheKey, RouteCacheKeyValidationError};
pub use route_cache::{RouteCache, RouteCacheError};
pub use route_metrics::{RouteMetrics, RouteMetricsError};
pub use route_queue::{JobDispatchError, RouteQueue};
pub use route_repository::{RoutePersistenceError, RouteRepository};
pub use user_repository::{UserPersistenceError, UserRepository};

#[cfg(test)]
mod tests;
