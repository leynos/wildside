//! Domain events emitted by user onboarding flows.
//!
//! These events stay transport agnostic so inbound adapters can map them to
//! protocol-specific payloads (e.g., WebSocket JSON envelopes) without
//! re-encoding domain logic.

use super::TraceId;
use crate::domain::user::User;
use crate::domain::user::UserValidationError;

/// Event emitted when a user record is created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserCreatedEvent {
    /// Correlation identifier provided by the caller.
    pub trace_id: TraceId,
    /// Domain representation of the new user.
    pub user: User,
}

/// Event emitted when a display name submission fails validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayNameRejectedEvent {
    /// Correlation identifier provided by the caller.
    pub trace_id: TraceId,
    /// Raw display name supplied by the client.
    pub attempted_name: String,
    /// Validation error that caused rejection.
    pub error: UserValidationError,
}

impl DisplayNameRejectedEvent {
    /// Stable field path for the rejection details.
    #[must_use]
    pub const fn field(&self) -> &'static str {
        "displayName"
    }

    /// Machine-readable code for the rejection, if available.
    #[must_use]
    pub fn code(&self) -> &'static str {
        self.error
            .display_name_error_meta()
            .map(|(code, _, _)| code)
            .unwrap_or("invalid")
    }

    /// Human readable message for the rejection.
    #[must_use]
    pub fn message(&self) -> String {
        self.error
            .display_name_error_meta()
            .map(|(_, message, _)| message)
            .unwrap_or_else(|| "Invalid display name.".to_owned())
    }
}

/// User lifecycle domain events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserEvent {
    /// A user has been created.
    UserCreated(UserCreatedEvent),
    /// A display name submission failed validation.
    DisplayNameRejected(DisplayNameRejectedEvent),
}

#[cfg(test)]
mod tests {
    //! Regression coverage for this module.
    use super::*;
    use crate::domain::user::{DISPLAY_NAME_MAX, DISPLAY_NAME_MIN};
    use rstest::rstest;

    #[rstest]
    fn maps_validation_errors_to_reasons_and_messages() {
        let empty = UserValidationError::EmptyDisplayName.display_name_error_meta();
        assert_eq!(
            empty,
            Some((
                "empty",
                "Display name must not be empty.".to_owned(),
                "displayName"
            ))
        );
        let too_short = UserValidationError::DisplayNameTooShort {
            min: DISPLAY_NAME_MIN,
        }
        .display_name_error_meta();
        assert_eq!(
            too_short,
            Some((
                "too_short",
                format!("Display name must be at least {DISPLAY_NAME_MIN} characters."),
                "displayName"
            ))
        );
        let too_long = UserValidationError::DisplayNameTooLong {
            max: DISPLAY_NAME_MAX,
        }
        .display_name_error_meta();
        assert_eq!(
            too_long,
            Some((
                "too_long",
                format!("Display name must be at most {DISPLAY_NAME_MAX} characters."),
                "displayName"
            ))
        );
        let invalid_chars =
            UserValidationError::DisplayNameInvalidCharacters.display_name_error_meta();
        assert_eq!(
            invalid_chars,
            Some((
                "invalid_chars",
                "Only alphanumeric characters, spaces, and underscores are allowed.".to_owned(),
                "displayName"
            ))
        );
        assert!(
            UserValidationError::InvalidId
                .display_name_error_meta()
                .is_none()
        );
    }
}
