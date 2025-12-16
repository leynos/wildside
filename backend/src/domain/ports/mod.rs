//! Domain ports and supporting types for the hexagonal boundary.

mod macros;
pub(crate) use macros::define_port_error;

mod cache_key;
mod login_service;
mod route_cache;
mod route_metrics;
mod route_queue;
mod route_repository;
mod user_onboarding;
mod user_repository;
mod users_query;

pub use cache_key::{RouteCacheKey, RouteCacheKeyValidationError};
pub use login_service::{FixtureLoginService, LoginService};
pub use route_cache::{RouteCache, RouteCacheError};
pub use route_metrics::{RouteMetrics, RouteMetricsError};
pub use route_queue::{JobDispatchError, RouteQueue};
pub use route_repository::{RoutePersistenceError, RouteRepository};
pub use user_onboarding::UserOnboarding;
pub use user_repository::{UserPersistenceError, UserRepository};
pub use users_query::{FixtureUsersQuery, UsersQuery};

#[cfg(test)]
mod tests;
