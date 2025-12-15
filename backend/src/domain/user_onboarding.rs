//! Domain service orchestrating user onboarding events.
//!
//! The service owns validation and event production so inbound adapters only
//! translate domain events into transport payloads.

use crate::domain::ports::UserOnboarding;
use crate::domain::user::{DisplayName, User, UserId, UserValidationError};
use crate::domain::user_events::{DisplayNameRejectedEvent, UserCreatedEvent, UserEvent};
use crate::middleware::trace::TraceId;

/// Stateless user onboarding service.
#[derive(Debug, Default, Clone, Copy)]
pub struct UserOnboardingService;

impl UserOnboardingService {
    /// Validate a display name and emit the appropriate domain event.
    #[must_use]
    pub fn register(&self, trace_id: TraceId, display_name: impl Into<String>) -> UserEvent {
        let display_name = display_name.into();
        match DisplayName::new(display_name.clone()) {
            Ok(display_name) => {
                let user = User::new(UserId::random(), display_name);
                UserEvent::UserCreated(UserCreatedEvent { trace_id, user })
            }
            Err(error) => {
                UserEvent::DisplayNameRejected(Self::build_rejection(trace_id, display_name, error))
            }
        }
    }

    pub(crate) fn build_rejection(
        trace_id: TraceId,
        attempted_name: String,
        error: UserValidationError,
    ) -> DisplayNameRejectedEvent {
        DisplayNameRejectedEvent {
            trace_id,
            attempted_name,
            error,
        }
    }
}

impl UserOnboarding for UserOnboardingService {
    fn register(&self, trace_id: TraceId, display_name: String) -> UserEvent {
        UserOnboardingService::register(self, trace_id, display_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::user::{DISPLAY_NAME_MAX, DISPLAY_NAME_MIN};
    use rstest::rstest;
    use rstest_bdd_macros::{given, then, when};
    use uuid::Uuid;

    /// Expected values for rejection assertions
    struct ExpectedRejection<'a> {
        code: &'a str,
        message: &'a str,
    }

    #[given("a valid display name submission")]
    fn a_valid_display_name_submission() -> (TraceId, String) {
        (TraceId::from_uuid(Uuid::nil()), "Ada Lovelace".into())
    }

    #[given("an invalid display name submission")]
    fn an_invalid_display_name_submission() -> (TraceId, String) {
        (TraceId::from_uuid(Uuid::nil()), "@da!".into())
    }

    #[when("the onboarding service registers it")]
    fn the_onboarding_service_registers_it(input: (TraceId, String)) -> UserEvent {
        let service = UserOnboardingService;
        let (trace_id, display_name) = input;
        service.register(trace_id, display_name)
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
        assert_eq!(rejected.code(), "invalid_chars");
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

    fn assert_rejection_case(
        trace_id: TraceId,
        attempted: &str,
        error: UserValidationError,
        expected: ExpectedRejection<'_>,
    ) {
        let rejection =
            UserOnboardingService::build_rejection(trace_id, attempted.to_owned(), error);
        assert_eq!(rejection.code(), expected.code);
        assert_eq!(rejection.message(), expected.message);
        assert_eq!(rejection.attempted_name, attempted);
    }

    #[rstest]
    fn build_rejection_maps_reason_and_message() {
        let base = (TraceId::from_uuid(Uuid::nil()), "bad".to_owned());

        assert_rejection_case(
            base.0,
            &base.1,
            UserValidationError::DisplayNameTooShort {
                min: DISPLAY_NAME_MIN,
            },
            ExpectedRejection {
                code: "too_short",
                message: "Display name must be at least 3 characters.",
            },
        );

        assert_rejection_case(
            base.0,
            &base.1,
            UserValidationError::DisplayNameTooLong {
                max: DISPLAY_NAME_MAX,
            },
            ExpectedRejection {
                code: "too_long",
                message: "Display name must be at most 32 characters.",
            },
        );

        assert_rejection_case(
            base.0,
            &base.1,
            UserValidationError::DisplayNameInvalidCharacters,
            ExpectedRejection {
                code: "invalid_chars",
                message: "Only alphanumeric characters, spaces, and underscores are allowed.",
            },
        );

        assert_rejection_case(
            base.0,
            &base.1,
            UserValidationError::EmptyDisplayName,
            ExpectedRejection {
                code: "empty",
                message: "Display name must not be empty.",
            },
        );
    }

    #[rstest]
    #[case("ab", "too_short")]
    #[case(&"a".repeat(DISPLAY_NAME_MAX + 1), "too_long")]
    fn rejects_length_boundaries(#[case] display_name: &str, #[case] expected_code: &str) {
        let service = UserOnboardingService;
        let event = service.register(TraceId::from_uuid(Uuid::nil()), display_name.to_owned());
        let rejected = match event {
            UserEvent::DisplayNameRejected(event) => event,
            _ => panic!("expected rejection event"),
        };
        assert_eq!(rejected.code(), expected_code);
    }
}
