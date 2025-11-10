//! Port abstraction for user persistence adapters and their errors.
use async_trait::async_trait;

use crate::domain::{User, UserId};

use super::define_port_error;

define_port_error! {
    /// Persistence errors raised by user repository adapters.
    pub enum UserPersistenceError {
        /// Repository connection could not be established.
        Connection { message: String } => "user repository connection failed: {message}",
        /// Query or mutation failed during execution.
        Query { message: String } => "user repository query failed: {message}",
    }
}

#[async_trait]
pub trait UserRepository: Send + Sync {
    /// Insert or update a user record.
    async fn upsert(&self, user: &User) -> Result<(), UserPersistenceError>;

    /// Fetch a user by identifier.
    async fn find_by_id(&self, id: &UserId) -> Result<Option<User>, UserPersistenceError>;
}
