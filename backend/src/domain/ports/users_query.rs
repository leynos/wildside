//! Driving port for user-facing queries.
//!
//! Inbound adapters (HTTP handlers) use this port to fetch user-visible data
//! without importing outbound persistence concerns. Production can back this
//! port with a repository + mapping layer; tests can use a deterministic
//! in-memory implementation.

use async_trait::async_trait;

use crate::domain::ports::ListUsersPageRequest;
use crate::domain::{DisplayName, Error, User, UserId};

/// Domain users page returned by the user-list query port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsersPage {
    rows: Vec<User>,
    has_more: bool,
}

impl UsersPage {
    /// Build a users page from rows and an overflow flag.
    ///
    /// # Examples
    ///
    /// ```
    /// use backend::domain::ports::UsersPage;
    ///
    /// let page = UsersPage::new(Vec::new(), false);
    /// assert!(!page.has_more());
    /// assert!(page.rows().is_empty());
    /// ```
    #[must_use]
    pub const fn new(rows: Vec<User>, has_more: bool) -> Self {
        Self { rows, has_more }
    }

    /// Borrow the users in this page.
    #[must_use]
    pub fn rows(&self) -> &[User] {
        &self.rows
    }

    /// Consume the page and return its users.
    #[must_use]
    pub fn into_rows(self) -> Vec<User> {
        self.rows
    }

    /// Whether another page exists in the requested direction.
    #[must_use]
    pub const fn has_more(&self) -> bool {
        self.has_more
    }
}

/// Domain use-case port for listing users.
#[async_trait]
pub trait UsersQuery: Send + Sync {
    /// Return the visible users list for the authenticated user.
    async fn list_users(&self, authenticated_user: &UserId) -> Result<Vec<User>, Error>;

    /// Return one keyset-ordered users page for the authenticated user.
    async fn list_users_page(
        &self,
        _authenticated_user: &UserId,
        _request: ListUsersPageRequest,
    ) -> Result<UsersPage, Error> {
        Err(Error::internal("paginated users query is not implemented"))
    }
}

/// Temporary fixture users query used until persistence is wired.
#[derive(Debug, Default, Clone, Copy)]
pub struct FixtureUsersQuery;

#[async_trait]
impl UsersQuery for FixtureUsersQuery {
    async fn list_users(&self, _authenticated_user: &UserId) -> Result<Vec<User>, Error> {
        Ok(vec![fixture_user()?])
    }

    async fn list_users_page(
        &self,
        _authenticated_user: &UserId,
        request: ListUsersPageRequest,
    ) -> Result<UsersPage, Error> {
        if request.cursor().is_some() {
            return Ok(UsersPage::new(Vec::new(), false));
        }

        Ok(UsersPage::new(vec![fixture_user()?], false))
    }
}

fn fixture_user() -> Result<User, Error> {
    const FIXTURE_ID: &str = "3fa85f64-5717-4562-b3fc-2c963f66afa6";
    const FIXTURE_DISPLAY_NAME: &str = "Ada Lovelace";

    // These values are compile-time constants; surface invalid data as an
    // internal error so automated checks catch accidental regressions.
    let id = UserId::new(FIXTURE_ID)
        .map_err(|err| Error::internal(format!("invalid fixture user id: {err}")))?;
    let display_name = DisplayName::new(FIXTURE_DISPLAY_NAME)
        .map_err(|err| Error::internal(format!("invalid fixture display name: {err}")))?;
    Ok(User::with_current_timestamp(id, display_name))
}

#[cfg(test)]
mod tests {
    //! Ensures the fixture users query returns the expected static user.
    use super::*;

    #[tokio::test]
    async fn fixture_users_query_returns_expected_user() {
        let query = FixtureUsersQuery;
        let user_id = UserId::new("11111111-1111-1111-1111-111111111111").expect("fixture user id");

        let users = query.list_users(&user_id).await.expect("users list");
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].display_name().as_ref(), "Ada Lovelace");
    }

    #[tokio::test]
    async fn fixture_users_query_returns_first_paginated_page() {
        let query = FixtureUsersQuery;
        let user_id = UserId::new("11111111-1111-1111-1111-111111111111").expect("fixture user id");
        let request = ListUsersPageRequest::new(None, 20);

        let page = query
            .list_users_page(&user_id, request)
            .await
            .expect("users page");

        assert_eq!(page.rows().len(), 1);
        assert!(!page.has_more());
        assert_eq!(page.rows()[0].display_name().as_ref(), "Ada Lovelace");
    }
}
