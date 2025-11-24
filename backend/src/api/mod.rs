//! REST API modules.

pub mod error;
pub mod health;
pub mod users;

pub use error::{ApiError, ApiResult};
