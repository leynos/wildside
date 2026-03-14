//! Driving port for user interest selection updates.
//!
//! Inbound adapters use this port to persist the user's interest theme
//! selections without reaching into persistence or caches directly.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::domain::{Error, InterestThemeId, UserId, UserInterests};

/// Request to replace a user's interest selections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserInterestsRequest {
    /// The user whose interests are being updated.
    pub user_id: UserId,
    /// Selected interest theme IDs.
    pub interest_theme_ids: Vec<InterestThemeId>,
    /// Expected aggregate revision for optimistic concurrency.
    pub expected_revision: Option<u32>,
}

/// Domain use-case port for updating a user's interest theme selections.
#[async_trait]
pub trait UserInterestsCommand: Send + Sync {
    /// Replace the current selections with the provided list.
    async fn set_interests(
        &self,
        request: UpdateUserInterestsRequest,
    ) -> Result<UserInterests, Error>;
}

/// Temporary fixture implementation used until persistence is wired.
#[derive(Debug, Default, Clone, Copy)]
pub struct FixtureUserInterestsCommand;

#[async_trait]
impl UserInterestsCommand for FixtureUserInterestsCommand {
    async fn set_interests(
        &self,
        request: UpdateUserInterestsRequest,
    ) -> Result<UserInterests, Error> {
        Ok(UserInterests::new(
            request.user_id,
            request.interest_theme_ids,
            request.expected_revision.map_or(1, |revision| revision + 1),
        ))
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
            .set_interests(UpdateUserInterestsRequest {
                user_id: user_id.clone(),
                interest_theme_ids: vec![interest_id.clone()],
                expected_revision: Some(3),
            })
            .await
            .expect("interests response");

        assert_eq!(interests.user_id(), &user_id);
        assert_eq!(interests.interest_theme_ids(), &[interest_id]);
        assert_eq!(interests.revision(), 4);
    }
}
