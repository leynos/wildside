//! Backend library modules.

pub mod api;
pub mod doc;
mod middleware;
pub use middleware::Trace;
pub mod models;
pub mod ws;

/// Public OpenAPI surface used by Swagger UI and tooling.
pub use doc::ApiDoc;
