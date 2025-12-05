//! Domain events emitted by user onboarding flows.
//!
//! These events stay transport agnostic so inbound adapters can map them to
//! protocol-specific payloads (e.g., WebSocket JSON envelopes) without
//! re-encoding domain logic.

use crate::domain::user::{User, UserValidationError, DISPLAY_NAME_MAX, DISPLAY_NAME_MIN};
use crate::middleware::trace::TraceId;

/// Normalised reasons a display name submission can be rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayNameRejectionReason {
    Empty,
    TooShort,
    TooLong,
    InvalidCharacters,
}

impl DisplayNameRejectionReason {
    /// Machine-readable rejection code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::TooShort => "too_short",
            Self::TooLong => "too_long",
            Self::InvalidCharacters => "invalid_chars",
        }
    }

    /// Human-readable policy message shared across adapters.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Empty => "Display name must not be empty.".to_owned(),
            Self::TooShort => {
                format!("Display name must be at least {DISPLAY_NAME_MIN} characters.",)
            }
            Self::TooLong => {
                format!("Display name must be at most {DISPLAY_NAME_MAX} characters.")
            }
            Self::InvalidCharacters => {
                "Only alphanumeric characters, spaces, and underscores are allowed.".to_owned()
            }
        }
    }

    pub(crate) fn from_validation_error(
        err: &UserValidationError,
    ) -> Option<DisplayNameRejectionReason> {
        match err {
            UserValidationError::EmptyDisplayName => Some(Self::Empty),
            UserValidationError::DisplayNameTooShort { .. } => Some(Self::TooShort),
            UserValidationError::DisplayNameTooLong { .. } => Some(Self::TooLong),
            UserValidationError::DisplayNameInvalidCharacters => Some(Self::InvalidCharacters),
            _ => None,
        }
    }
}

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
    /// Normalised reason for rejection.
    pub reason: DisplayNameRejectionReason,
    /// Human-friendly error message.
    pub message: String,
}

impl DisplayNameRejectedEvent {
    /// Stable field path for the rejection details.
    #[must_use]
    pub const fn field(&self) -> &'static str {
        "displayName"
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
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn maps_validation_errors_to_reasons_and_messages() {
        assert_eq!(
            DisplayNameRejectionReason::from_validation_error(
                &UserValidationError::EmptyDisplayName
            ),
            Some(DisplayNameRejectionReason::Empty)
        );
        assert_eq!(
            DisplayNameRejectionReason::from_validation_error(
                &UserValidationError::DisplayNameTooShort {
                    min: DISPLAY_NAME_MIN
                },
            ),
            Some(DisplayNameRejectionReason::TooShort)
        );
        assert_eq!(
            DisplayNameRejectionReason::from_validation_error(
                &UserValidationError::DisplayNameTooLong {
                    max: DISPLAY_NAME_MAX
                },
            ),
            Some(DisplayNameRejectionReason::TooLong)
        );
        assert_eq!(
            DisplayNameRejectionReason::from_validation_error(
                &UserValidationError::DisplayNameInvalidCharacters,
            ),
            Some(DisplayNameRejectionReason::InvalidCharacters)
        );
        assert!(
            DisplayNameRejectionReason::from_validation_error(&UserValidationError::InvalidId)
                .is_none()
        );

        assert_eq!(DisplayNameRejectionReason::Empty.code(), "empty");
        assert_eq!(
            DisplayNameRejectionReason::TooShort.message(),
            format!("Display name must be at least {DISPLAY_NAME_MIN} characters."),
        );
        assert_eq!(
            DisplayNameRejectionReason::TooLong.message(),
            format!("Display name must be at most {DISPLAY_NAME_MAX} characters."),
        );
        assert_eq!(
            DisplayNameRejectionReason::InvalidCharacters.message(),
            "Only alphanumeric characters, spaces, and underscores are allowed.".to_owned()
        );
    }
}
