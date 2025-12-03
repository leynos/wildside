//! HTTP inbound adapter exposing REST endpoints.

pub mod auth;
pub mod error;
pub mod health;
pub mod session;
#[cfg(test)]
pub mod test_utils;
pub mod users;

pub use error::ApiResult;
