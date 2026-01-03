//! Driving port for user preferences queries.
//!
//! Inbound adapters (HTTP handlers) use this port to fetch user preferences
//! without importing outbound persistence concerns. Implementations should
//! ensure default preferences exist when none have been stored yet.

use async_trait::async_trait;

use crate::domain::{Error, UserId, UserPreferences};

/// Domain use-case port for fetching user preferences.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait UserPreferencesQuery: Send + Sync {
    /// Fetch preferences for the authenticated user.
    async fn fetch_preferences(&self, user_id: &UserId) -> Result<UserPreferences, Error>;
}

/// Fixture query returning default preferences.
#[derive(Debug, Default, Clone, Copy)]
pub struct FixtureUserPreferencesQuery;

#[async_trait]
impl UserPreferencesQuery for FixtureUserPreferencesQuery {
    async fn fetch_preferences(&self, user_id: &UserId) -> Result<UserPreferences, Error> {
        Ok(UserPreferences::new_default(user_id.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fixture_query_returns_default_preferences() {
        let query = FixtureUserPreferencesQuery;
        let user_id = UserId::random();

        let prefs = query
            .fetch_preferences(&user_id)
            .await
            .expect("preferences fetched");

        assert_eq!(prefs.user_id, user_id);
        assert_eq!(prefs.revision, 1);
    }
}
