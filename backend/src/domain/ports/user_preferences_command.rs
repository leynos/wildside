//! Driving port for user preferences operations.
//!
//! The [`UserPreferencesCommand`] trait defines the inbound contract for
//! updating user preferences. HTTP handlers and other adapters call this port
//! to modify preferences, with support for idempotency and optimistic
//! concurrency.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{Error, IdempotencyKey, UnitSystem, UserId, UserPreferences};

/// Request to update user preferences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePreferencesRequest {
    /// The user whose preferences are being updated.
    pub user_id: UserId,
    /// Selected interest theme IDs.
    pub interest_theme_ids: Vec<Uuid>,
    /// Enabled safety toggle IDs.
    pub safety_toggle_ids: Vec<Uuid>,
    /// Display unit system.
    pub unit_system: UnitSystem,
    /// Expected revision for optimistic concurrency.
    ///
    /// - `None` for first-time saves (no existing preferences).
    /// - `Some(n)` to ensure the update only succeeds if the current revision
    ///   is `n`.
    pub expected_revision: Option<u32>,
    /// Optional idempotency key for safe retries.
    pub idempotency_key: Option<IdempotencyKey>,
}

/// Response from updating preferences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePreferencesResponse {
    /// The updated preferences with new revision.
    pub preferences: UserPreferences,
    /// Whether this response was replayed from a previous idempotent request.
    pub replayed: bool,
}

/// Driving port for user preferences operations.
///
/// This port is consumed by inbound adapters (e.g., HTTP handlers) to update
/// user preferences. Implementations coordinate between the preferences
/// repository and idempotency repository to provide safe, retryable updates.
///
/// # Idempotency
///
/// When an `idempotency_key` is provided, the implementation should:
/// 1. Check if a response for this key already exists.
/// 2. If so, return the cached response with `replayed: true`.
/// 3. If not, perform the update and cache the response.
///
/// # Optimistic Concurrency
///
/// When `expected_revision` is provided, the update should fail with a
/// conflict error if the current revision doesn't match.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait UserPreferencesCommand: Send + Sync {
    /// Update user preferences with idempotency and revision check.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The revision check fails (conflict).
    /// - The idempotency key was used with a different payload (conflict).
    /// - A database or connection error occurs.
    async fn update(
        &self,
        request: UpdatePreferencesRequest,
    ) -> Result<UpdatePreferencesResponse, Error>;
}

/// Fixture implementation for testing.
///
/// Always returns default preferences without persisting anything.
#[derive(Debug, Default, Clone, Copy)]
pub struct FixtureUserPreferencesCommand;

#[async_trait]
impl UserPreferencesCommand for FixtureUserPreferencesCommand {
    async fn update(
        &self,
        request: UpdatePreferencesRequest,
    ) -> Result<UpdatePreferencesResponse, Error> {
        let preferences = UserPreferences::builder(request.user_id)
            .interest_theme_ids(request.interest_theme_ids)
            .safety_toggle_ids(request.safety_toggle_ids)
            .unit_system(request.unit_system)
            .revision(request.expected_revision.map_or(1, |r| r + 1))
            .build();

        Ok(UpdatePreferencesResponse {
            preferences,
            replayed: false,
        })
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for this module.
    use super::*;

    #[tokio::test]
    async fn fixture_command_returns_preferences() {
        let command = FixtureUserPreferencesCommand;
        let request = UpdatePreferencesRequest {
            user_id: UserId::random(),
            interest_theme_ids: vec![Uuid::new_v4()],
            safety_toggle_ids: vec![],
            unit_system: UnitSystem::Imperial,
            expected_revision: None,
            idempotency_key: None,
        };

        let response = command.update(request).await.expect("should succeed");

        assert!(!response.replayed);
        assert_eq!(response.preferences.revision, 1);
        assert_eq!(response.preferences.unit_system, UnitSystem::Imperial);
    }

    #[tokio::test]
    async fn fixture_command_increments_revision() {
        let command = FixtureUserPreferencesCommand;
        let request = UpdatePreferencesRequest {
            user_id: UserId::random(),
            interest_theme_ids: vec![],
            safety_toggle_ids: vec![],
            unit_system: UnitSystem::Metric,
            expected_revision: Some(3),
            idempotency_key: None,
        };

        let response = command.update(request).await.expect("should succeed");

        assert_eq!(response.preferences.revision, 4);
    }
}
