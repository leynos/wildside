//! Driving port for user interest selection updates.
//!
//! Inbound adapters use this port to persist the user's interest theme
//! selections without reaching into persistence or caches directly.

use async_trait::async_trait;

use crate::domain::{Error, InterestThemeId, UserId, UserInterests};

/// Domain use-case port for updating a user's interest theme selections.
#[async_trait]
pub trait UserInterestsCommand: Send + Sync {
    /// Replace the current selections with the provided list.
    async fn set_interests(
        &self,
        user_id: &UserId,
        interest_theme_ids: Vec<InterestThemeId>,
    ) -> Result<UserInterests, Error>;
}

/// Temporary fixture implementation used until persistence is wired.
#[derive(Debug, Default, Clone, Copy)]
pub struct FixtureUserInterestsCommand;

#[async_trait]
impl UserInterestsCommand for FixtureUserInterestsCommand {
    async fn set_interests(
        &self,
        user_id: &UserId,
        interest_theme_ids: Vec<InterestThemeId>,
    ) -> Result<UserInterests, Error> {
        Ok(UserInterests::new(user_id.clone(), interest_theme_ids))
    }
}

#[cfg(test)]
mod tests {
    //! Checks the fixture command echoes user interest updates unchanged.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[tokio::test]
    async fn fixture_interests_command_echoes_payload() {
        let command = FixtureUserInterestsCommand;
        let user_id = UserId::new("11111111-1111-1111-1111-111111111111").expect("user id");
        let interest_id =
            InterestThemeId::new("3fa85f64-5717-4562-b3fc-2c963f66afa6").expect("interest id");

        let interests = command
            .set_interests(&user_id, vec![interest_id.clone()])
            .await
            .expect("interests response");

        assert_eq!(interests.user_id(), &user_id);
        assert_eq!(interests.interest_theme_ids(), &[interest_id]);
    }
}
