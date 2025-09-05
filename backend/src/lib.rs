//! Backend library modules.

pub mod api;
pub mod doc;
pub mod middleware;
pub mod models;
pub mod ws;

/// Public OpenAPI surface used by Swagger UI and tooling.
pub use doc::ApiDoc;
