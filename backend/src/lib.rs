#![cfg_attr(not(any(test, doctest)), deny(clippy::unwrap_used))]
#![cfg_attr(not(any(test, doctest)), deny(clippy::expect_used))]
//! Backend library modules.
//!
//! Structure follows the hexagonal layout: inbound adapters (HTTP/WebSocket),
//! domain, and outbound adapters (persistence, cache, queue).
extern crate self as backend;

pub mod doc;
pub mod domain;
pub mod er_snapshots;
#[cfg(feature = "example-data")]
pub mod example_data;
pub mod inbound;
mod middleware;
pub mod outbound;
pub mod server;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
pub use domain::TraceId;
pub use middleware::Trace;

/// Public OpenAPI surface used by Swagger UI and tooling.
pub use doc::ApiDoc;
pub use domain::ProcessHealth;
pub use inbound::http;
pub use inbound::http::error::ApiResult;

/// Register optional pagination metrics for the HTTP metrics endpoint.
#[cfg(feature = "metrics")]
pub fn register_pagination_error_metrics(
    registry: &prometheus::Registry,
) -> Result<(), prometheus::Error> {
    domain::pagination_errors::register_pagination_error_metrics(registry)
}
