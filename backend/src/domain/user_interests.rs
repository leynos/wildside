//! User interest selections.
//!
//! Purpose: capture the current set of interest themes chosen by a user.

use serde::{Deserialize, Serialize};

use crate::domain::{InterestThemeId, UserId};

/// Interest theme selections for a user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct UserInterests {
    user_id: UserId,
    interest_theme_ids: Vec<InterestThemeId>,
    revision: u32,
}

impl UserInterests {
    /// Build a new [`UserInterests`] value.
    pub fn new(user_id: UserId, interest_theme_ids: Vec<InterestThemeId>, revision: u32) -> Self {
        Self {
            user_id,
            interest_theme_ids,
            revision,
        }
    }

    /// Stable identifier for the owning user.
    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    /// Selected interest theme identifiers.
    pub fn interest_theme_ids(&self) -> &[InterestThemeId] {
        &self.interest_theme_ids
    }

    /// Shared aggregate revision after the interests update.
    pub fn revision(&self) -> u32 {
        self.revision
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for this module.
    use super::*;

    #[test]
    fn user_interests_exposes_values() {
        let user_id = UserId::new("11111111-1111-1111-1111-111111111111").expect("user id");
        let interest_id =
            InterestThemeId::new("3fa85f64-5717-4562-b3fc-2c963f66afa6").expect("interest id");
        let interests = UserInterests::new(user_id.clone(), vec![interest_id.clone()], 2);

        assert_eq!(interests.user_id(), &user_id);
        assert_eq!(interests.interest_theme_ids(), &[interest_id]);
        assert_eq!(interests.revision(), 2);
    }
}
