//! HTTP inbound adapter exposing REST endpoints.

pub mod annotations;
pub mod cache_control;
pub mod catalogue;
pub mod error;
pub mod health;
pub mod idempotency;
pub mod offline;
pub mod preferences;
pub mod routes;
pub mod schemas;
pub mod session;
pub mod session_config;
pub mod state;
#[cfg(test)]
pub mod test_utils;
pub mod users;
pub mod validation;
pub mod walk_sessions;

pub use error::ApiResult;
