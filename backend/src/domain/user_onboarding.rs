//! Domain service orchestrating user onboarding events.
//!
//! The service owns validation and event production so inbound adapters only
//! translate domain events into transport payloads.

use crate::domain::user::{DisplayName, User, UserId, UserValidationError};
use crate::domain::user_events::{
    DisplayNameRejectedEvent, DisplayNameRejectionReason, UserCreatedEvent, UserEvent,
};
use crate::middleware::trace::TraceId;

/// Command carrying the client's desired display name and correlation id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayNameSubmission {
    /// Trace identifier supplied by the caller.
    pub trace_id: TraceId,
    /// Raw display name provided by the caller.
    pub display_name: String,
}

impl DisplayNameSubmission {
    /// Build a new submission from raw components.
    #[must_use]
    pub fn new(trace_id: TraceId, display_name: impl Into<String>) -> Self {
        Self {
            trace_id,
            display_name: display_name.into(),
        }
    }
}

/// Stateless user onboarding service.
#[derive(Debug, Default, Clone, Copy)]
pub struct UserOnboardingService;

impl UserOnboardingService {
    /// Validate a display name and emit the appropriate domain event.
    #[must_use]
    pub fn register(&self, submission: DisplayNameSubmission) -> UserEvent {
        match DisplayName::new(submission.display_name.clone()) {
            Ok(display_name) => {
                let user = User::new(UserId::random(), display_name);
                UserEvent::UserCreated(UserCreatedEvent {
                    trace_id: submission.trace_id,
                    user,
                })
            }
            Err(error) => UserEvent::DisplayNameRejected(Self::build_rejection(submission, error)),
        }
    }

    fn build_rejection(
        submission: DisplayNameSubmission,
        error: UserValidationError,
    ) -> DisplayNameRejectedEvent {
        let reason = DisplayNameRejectionReason::from_validation_error(&error)
            .unwrap_or(DisplayNameRejectionReason::InvalidCharacters);
        DisplayNameRejectedEvent {
            trace_id: submission.trace_id,
            attempted_name: submission.display_name,
            reason,
            message: reason.message(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::user::DISPLAY_NAME_MAX;
    use rstest::rstest;
    use rstest_bdd_macros::{given, then, when};
    use uuid::Uuid;

    #[given("a valid display name submission")]
    fn a_valid_display_name_submission() -> DisplayNameSubmission {
        DisplayNameSubmission::new(TraceId::from_uuid(Uuid::nil()), "Ada Lovelace")
    }

    #[given("an invalid display name submission")]
    fn an_invalid_display_name_submission() -> DisplayNameSubmission {
        DisplayNameSubmission::new(TraceId::from_uuid(Uuid::nil()), "@da!")
    }

    #[when("the onboarding service registers it")]
    fn the_onboarding_service_registers_it(submission: DisplayNameSubmission) -> UserEvent {
        let service = UserOnboardingService;
        service.register(submission)
    }

    #[then("a user created event is emitted")]
    fn a_user_created_event_is_emitted(event: UserEvent) {
        let created = match event {
            UserEvent::UserCreated(event) => event,
            other => panic!("expected UserCreated event, got {other:?}"),
        };
        assert_eq!(created.user.display_name().as_ref(), "Ada Lovelace");
    }

    #[then("a display name rejection event is emitted")]
    fn a_display_name_rejection_event_is_emitted(event: UserEvent) {
        let rejected = match event {
            UserEvent::DisplayNameRejected(event) => event,
            other => panic!("expected DisplayNameRejected event, got {other:?}"),
        };
        assert_eq!(rejected.reason.code(), "invalid_chars");
        assert_eq!(rejected.attempted_name, "@da!");
    }

    #[rstest]
    fn emits_user_created_event_on_valid_submission() {
        let submission = a_valid_display_name_submission();
        let event = the_onboarding_service_registers_it(submission);
        a_user_created_event_is_emitted(event);
    }

    #[rstest]
    fn emits_rejection_on_invalid_submission() {
        let submission = an_invalid_display_name_submission();
        let event = the_onboarding_service_registers_it(submission);
        a_display_name_rejection_event_is_emitted(event);
    }

    #[rstest]
    #[case("ab", "too_short")]
    #[case(&"a".repeat(DISPLAY_NAME_MAX + 1), "too_long")]
    fn rejects_length_boundaries(#[case] display_name: &str, #[case] expected_code: &str) {
        let service = UserOnboardingService;
        let submission =
            DisplayNameSubmission::new(TraceId::from_uuid(Uuid::nil()), display_name.to_owned());
        let event = service.register(submission);
        let rejected = match event {
            UserEvent::DisplayNameRejected(event) => event,
            _ => panic!("expected rejection event"),
        };
        assert_eq!(rejected.reason.code(), expected_code);
    }
}
