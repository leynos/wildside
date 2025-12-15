//! Driving port for user-facing queries.
//!
//! Inbound adapters (HTTP handlers) use this port to fetch user-visible data
//! without importing outbound persistence concerns. Production can back this
//! port with a repository + mapping layer; tests can use a deterministic
//! in-memory implementation.

use async_trait::async_trait;

use crate::domain::{DisplayName, Error, User, UserId};

/// Domain use-case port for listing users.
#[async_trait]
pub trait UsersQuery: Send + Sync {
    /// Return the visible users list for the authenticated user.
    async fn list_users(&self, authenticated_user: &UserId) -> Result<Vec<User>, Error>;
}

/// Temporary fixture users query used until persistence is wired.
#[derive(Debug, Default, Clone, Copy)]
pub struct FixtureUsersQuery;

#[async_trait]
impl UsersQuery for FixtureUsersQuery {
    async fn list_users(&self, _authenticated_user: &UserId) -> Result<Vec<User>, Error> {
        const FIXTURE_ID: &str = "3fa85f64-5717-4562-b3fc-2c963f66afa6";
        const FIXTURE_DISPLAY_NAME: &str = "Ada Lovelace";

        // These values are compile-time constants; surface invalid data as an
        // internal error so automated checks catch accidental regressions.
        let id = UserId::new(FIXTURE_ID)
            .map_err(|err| Error::internal(format!("invalid fixture user id: {err}")))?;
        let display_name = DisplayName::new(FIXTURE_DISPLAY_NAME)
            .map_err(|err| Error::internal(format!("invalid fixture display name: {err}")))?;
        Ok(vec![User::new(id, display_name)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[tokio::test]
    async fn fixture_users_query_returns_expected_user() {
        let query = FixtureUsersQuery;
        let user_id = UserId::new("11111111-1111-1111-1111-111111111111").expect("fixture user id");

        let users = query.list_users(&user_id).await.expect("users list");
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].display_name().as_ref(), "Ada Lovelace");
    }
}
