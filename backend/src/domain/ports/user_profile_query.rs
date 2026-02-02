//! Driving port for user profile queries.
//!
//! Inbound adapters use this port to load a user's profile without importing
//! persistence details. Fixture implementations keep HTTP handlers testable
//! before databases are wired.

use async_trait::async_trait;

use crate::domain::{DisplayName, Error, User, UserId};

/// Domain use-case port for reading the current user's profile.
#[async_trait]
pub trait UserProfileQuery: Send + Sync {
    /// Return the profile for the authenticated user.
    async fn fetch_profile(&self, user_id: &UserId) -> Result<User, Error>;
}

/// Temporary fixture profile query used until persistence is wired.
#[derive(Debug, Default, Clone, Copy)]
pub struct FixtureUserProfileQuery;

#[async_trait]
impl UserProfileQuery for FixtureUserProfileQuery {
    async fn fetch_profile(&self, user_id: &UserId) -> Result<User, Error> {
        let display_name = DisplayName::new("Ada Lovelace")
            .map_err(|err| Error::internal(format!("invalid fixture display name: {err}")))?;
        Ok(User::new(user_id.clone(), display_name))
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for this module.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[tokio::test]
    async fn fixture_profile_query_returns_requested_user() {
        let query = FixtureUserProfileQuery;
        let user_id = UserId::new("11111111-1111-1111-1111-111111111111").expect("user id");

        let user = query
            .fetch_profile(&user_id)
            .await
            .expect("profile response");
        assert_eq!(user.id(), &user_id);
        assert_eq!(user.display_name().as_ref(), "Ada Lovelace");
    }
}
