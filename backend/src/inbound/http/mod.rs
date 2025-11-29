//! HTTP inbound adapter exposing REST endpoints.

pub mod error;
pub mod health;
pub mod session;
pub mod users;

pub use error::ApiResult;
