use async_trait::async_trait;

use super::define_port_error;

define_port_error! {
    /// Errors surfaced by the persistence adapter when handling routes.
    pub enum RoutePersistenceError {
        /// Database connectivity or transaction failures.
        Connection { message: String } => "route persistence connection failed: {message}",
        /// Duplicate request identifiers detected.
        Conflict { request_id: String } => "route conflict detected for request {request_id}",
        /// Catch-all for write failures that bubble up from the adapter.
        Write { message: String } => "route persistence failed: {message}",
    }
}

#[async_trait]
pub trait RouteRepository: Send + Sync {
    /// Domain-specific plan representation.
    type Plan: Send + Sync;

    /// Persist a route plan.
    async fn save(&self, plan: &Self::Plan) -> Result<(), RoutePersistenceError>;

    /// Fetch a plan by its request identifier.
    async fn find_by_request_id(
        &self,
        request_id: &str,
    ) -> Result<Option<Self::Plan>, RoutePersistenceError>;
}
