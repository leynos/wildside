//! Test doubles for driving ports used by the adapter guardrails suite.

mod doubles_annotations;
mod doubles_preferences;
mod doubles_users;
mod doubles_ws;

pub(crate) use doubles_annotations::{
    DeleteNoteCommandResponse, RecordingRouteAnnotationsCommand, RecordingRouteAnnotationsQuery,
    RouteAnnotationsQueryResponse, UpdateProgressCommandResponse, UpsertNoteCommandResponse,
};
pub(crate) use doubles_preferences::{
    RecordingUserPreferencesCommand, RecordingUserPreferencesQuery, UserPreferencesCommandResponse,
    UserPreferencesQueryResponse,
};
pub(crate) use doubles_users::{
    LoginResponse, RecordingLoginService, RecordingUserInterestsCommand, RecordingUserProfileQuery,
    RecordingUsersQuery, UserInterestsResponse, UserProfileResponse, UsersResponse,
};
pub(crate) use doubles_ws::QueueUserOnboarding;
