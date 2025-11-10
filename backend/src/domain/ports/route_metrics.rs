//! Domain port surface for recording route cache hit/miss metrics.
use async_trait::async_trait;

use super::define_port_error;

define_port_error! {
    /// Errors exposed when recording metrics.
    pub enum RouteMetricsError {
        /// Metric exporter rejected the write.
        Export { message: String } => "route metrics exporter failed: {message}",
    }
}

#[async_trait]
pub trait RouteMetrics: Send + Sync {
    /// Record a cache hit.
    async fn record_cache_hit(&self) -> Result<(), RouteMetricsError>;

    /// Record a cache miss.
    async fn record_cache_miss(&self) -> Result<(), RouteMetricsError>;
}
