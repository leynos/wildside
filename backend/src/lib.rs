#![cfg_attr(not(any(test, doctest)), deny(clippy::unwrap_used))]
#![cfg_attr(not(any(test, doctest)), deny(clippy::expect_used))]
//! Backend library modules.
//!
//! Structure follows the hexagonal layout: inbound adapters (HTTP/WebSocket),
//! domain, and outbound adapters (persistence, cache, queue).

pub mod doc;
pub mod domain;
pub mod inbound;
mod middleware;
pub mod outbound;
pub use domain::TraceId;
pub use middleware::Trace;

/// Public OpenAPI surface used by Swagger UI and tooling.
pub use doc::ApiDoc;
pub use inbound::http;
pub use inbound::http::error::ApiResult;
pub use inbound::http::health::HealthState;
