//! Port abstraction for user persistence adapters and their errors.
use std::num::NonZeroUsize;

use async_trait::async_trait;
use pagination::Cursor;

use crate::domain::{User, UserCursorKey, UserId};

use super::define_port_error;

define_port_error! {
    /// Pagination errors caused by caller-provided page boundaries.
    pub enum UserPaginationError {
        /// Cursor text could not be decoded into the expected users cursor.
        InvalidCursorFormat { message: String } => "invalid users cursor: {message}",
        /// Cursor direction is not supported by the repository query.
        UnsupportedDirection { direction: String } => "unsupported users cursor direction: {direction}",
    }
}

define_port_error! {
    /// Persistence errors raised by user repository adapters.
    pub enum UserPersistenceError {
        /// Repository connection could not be established.
        Connection { message: String } => "user repository connection failed: {message}",
        /// Query or mutation failed during execution.
        Query { message: String } => "user repository query failed: {message}",
        /// Pagination request failed because the caller supplied an invalid page boundary.
        Pagination { error: UserPaginationError } => "user repository pagination failed: {error}",
    }
}

/// Request for a keyset-ordered page from the users table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListUsersPageRequest {
    cursor: Option<Cursor<UserCursorKey>>,
    limit: NonZeroUsize,
}

impl ListUsersPageRequest {
    /// Build a users page request from a cursor and caller-normalized limit.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::num::NonZeroUsize;
    ///
    /// use backend::domain::ports::ListUsersPageRequest;
    ///
    /// let limit = NonZeroUsize::new(20).expect("non-zero limit");
    /// let request = ListUsersPageRequest::new(None, limit);
    /// assert_eq!(request.limit(), 20);
    /// assert!(request.cursor().is_none());
    /// ```
    #[must_use]
    pub const fn new(cursor: Option<Cursor<UserCursorKey>>, limit: NonZeroUsize) -> Self {
        Self { cursor, limit }
    }

    /// Borrow the optional page boundary cursor.
    #[must_use]
    pub const fn cursor(&self) -> Option<&Cursor<UserCursorKey>> {
        self.cursor.as_ref()
    }

    /// Return the caller-normalized page size.
    #[must_use]
    pub const fn limit(&self) -> usize {
        self.limit.get()
    }

    /// Consume the request into its cursor and limit components.
    #[must_use]
    pub fn into_parts(self) -> (Option<Cursor<UserCursorKey>>, NonZeroUsize) {
        (self.cursor, self.limit)
    }
}

#[async_trait]
pub trait UserRepository: Send + Sync {
    /// Insert or update a user record.
    async fn upsert(&self, user: &User) -> Result<(), UserPersistenceError>;

    /// Fetch a user by identifier.
    async fn find_by_id(&self, id: &UserId) -> Result<Option<User>, UserPersistenceError>;

    /// Fetch a keyset-ordered users page.
    ///
    /// Implementations should fetch one more row than `request.limit()` when
    /// possible so the caller can detect whether another page exists. Returned
    /// rows remain in `(created_at ASC, id ASC)` order for both directions.
    async fn list_page(
        &self,
        _request: ListUsersPageRequest,
    ) -> Result<Vec<User>, UserPersistenceError> {
        Err(UserPersistenceError::query(
            "paginated user listing is not implemented",
        ))
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for user repository port error constructors.

    use super::{UserPaginationError, UserPersistenceError};

    #[test]
    fn pagination_error_constructors_preserve_client_failure_context() {
        let invalid = UserPaginationError::invalid_cursor_format("base64 decode failed");
        assert_eq!(
            invalid.to_string(),
            "invalid users cursor: base64 decode failed"
        );

        let unsupported = UserPaginationError::unsupported_direction("sideways");
        assert_eq!(
            unsupported.to_string(),
            "unsupported users cursor direction: sideways"
        );
    }

    #[test]
    fn persistence_error_can_wrap_pagination_failures() {
        let error = UserPersistenceError::pagination(UserPaginationError::unsupported_direction(
            "sideways",
        ));

        assert!(matches!(
            error,
            UserPersistenceError::Pagination {
                error: UserPaginationError::UnsupportedDirection { .. }
            }
        ));
    }
}
