//! HTTP inbound adapter exposing REST endpoints.

pub mod error;
pub mod health;
pub mod session;
pub mod state;
#[cfg(test)]
pub mod test_utils;
pub mod users;

pub use error::ApiResult;
