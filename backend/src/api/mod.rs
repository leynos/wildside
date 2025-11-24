//! REST API modules.

pub mod error;
pub mod health;
pub mod users;

pub use error::{map_domain_error, ApiError, ApiResult};
