#![cfg_attr(
    test,
    expect(clippy::expect_used, reason = "tests require contextual panics")
)]
#![cfg_attr(not(any(test, doctest)), deny(clippy::unwrap_used))]
#![cfg_attr(not(any(test, doctest)), deny(clippy::expect_used))]
//! Backend library modules.

pub mod api;
pub mod doc;
mod middleware;
pub use middleware::Trace;
pub mod models;
pub mod ws;

/// Public OpenAPI surface used by Swagger UI and tooling.
pub use doc::ApiDoc;
