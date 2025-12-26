//! HTTP inbound adapter exposing REST endpoints.

pub mod error;
pub mod health;
pub mod routes;
pub mod schemas;
pub mod session;
pub mod session_config;
pub mod state;
#[cfg(test)]
pub mod test_utils;
pub mod users;

pub use error::ApiResult;
