//! Test doubles for driving ports used by the adapter guardrails suite.

mod doubles_annotations;
mod doubles_catalogue;
mod doubles_offline_walk;
mod doubles_preferences;
mod doubles_users;
mod doubles_ws;
#[path = "recording_double_macro.rs"]
mod recording_double_macro;

pub(crate) use doubles_annotations::{
    DeleteNoteCommandResponse, RecordingRouteAnnotationsCommand, RecordingRouteAnnotationsQuery,
    RouteAnnotationsQueryResponse, UpdateProgressCommandResponse, UpsertNoteCommandResponse,
};
pub(crate) use doubles_catalogue::{
    CatalogueQueryResponse, DescriptorQueryResponse, RecordingCatalogueRepository,
    RecordingDescriptorRepository,
};
pub(crate) use doubles_offline_walk::{
    DeleteOfflineBundleCommandResponse, OfflineBundleGetQueryResponse,
    OfflineBundleListQueryResponse, RecordingOfflineBundleCommand, RecordingOfflineBundleQuery,
    RecordingWalkSessionCommand, UpsertOfflineBundleCommandResponse, WalkSessionCommandResponse,
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
