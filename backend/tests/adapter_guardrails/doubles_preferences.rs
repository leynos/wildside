//! Test doubles for user preferences driving ports.

use super::recording_double_macro::recording_double;
use backend::domain::ports::{
    UpdatePreferencesRequest, UpdatePreferencesResponse, UserPreferencesCommand,
    UserPreferencesQuery,
};
use backend::domain::{Error, UserId, UserPreferences};

recording_double! {
    /// Configurable success or failure outcome for RecordingUserPreferencesCommand.
    pub(crate) enum UserPreferencesCommandResponse {
        Ok(UpdatePreferencesResponse),
        Err(Error),
    }

    pub(crate) struct RecordingUserPreferencesCommand {
        calls: UpdatePreferencesRequest,
        trait: UserPreferencesCommand,
        method: update(&self, request: UpdatePreferencesRequest)
            -> Result<UpdatePreferencesResponse, Error>,
        record: request,
        calls_lock: "preferences command calls lock",
        response_lock: "preferences command response lock",
    }
}

recording_double! {
    /// Configurable success or failure outcome for RecordingUserPreferencesQuery.
    pub(crate) enum UserPreferencesQueryResponse {
        Ok(UserPreferences),
        Err(Error),
    }

    pub(crate) struct RecordingUserPreferencesQuery {
        calls: String,
        trait: UserPreferencesQuery,
        method: fetch_preferences(&self, user_id: &UserId) -> Result<UserPreferences, Error>,
        record: user_id.to_string(),
        calls_lock: "preferences query calls lock",
        response_lock: "preferences query response lock",
    }
}
