//! Test doubles for user preferences driving ports.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use backend::domain::ports::{
    UpdatePreferencesRequest, UpdatePreferencesResponse, UserPreferencesCommand,
    UserPreferencesQuery,
};
use backend::domain::{Error, UserId, UserPreferences};

/// Configurable success or failure outcome for RecordingUserPreferencesCommand.
#[derive(Clone)]
pub(crate) enum UserPreferencesCommandResponse {
    Ok(UpdatePreferencesResponse),
    Err(Error),
}

#[derive(Clone)]
pub(crate) struct RecordingUserPreferencesCommand {
    calls: Arc<Mutex<Vec<UpdatePreferencesRequest>>>,
    response: Arc<Mutex<UserPreferencesCommandResponse>>,
}

impl RecordingUserPreferencesCommand {
    pub(crate) fn new(response: UserPreferencesCommandResponse) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            response: Arc::new(Mutex::new(response)),
        }
    }

    pub(crate) fn calls(&self) -> Vec<UpdatePreferencesRequest> {
        self.calls
            .lock()
            .expect("preferences command calls lock")
            .clone()
    }

    pub(crate) fn set_response(&self, response: UserPreferencesCommandResponse) {
        *self
            .response
            .lock()
            .expect("preferences command response lock") = response;
    }
}

#[async_trait]
impl UserPreferencesCommand for RecordingUserPreferencesCommand {
    async fn update(
        &self,
        request: UpdatePreferencesRequest,
    ) -> Result<UpdatePreferencesResponse, Error> {
        self.calls
            .lock()
            .expect("preferences command calls lock")
            .push(request);
        match self
            .response
            .lock()
            .expect("preferences command response lock")
            .clone()
        {
            UserPreferencesCommandResponse::Ok(response) => Ok(response),
            UserPreferencesCommandResponse::Err(error) => Err(error),
        }
    }
}

/// Configurable success or failure outcome for RecordingUserPreferencesQuery.
#[derive(Clone)]
pub(crate) enum UserPreferencesQueryResponse {
    Ok(UserPreferences),
    Err(Error),
}

#[derive(Clone)]
pub(crate) struct RecordingUserPreferencesQuery {
    calls: Arc<Mutex<Vec<String>>>,
    response: Arc<Mutex<UserPreferencesQueryResponse>>,
}

impl RecordingUserPreferencesQuery {
    pub(crate) fn new(response: UserPreferencesQueryResponse) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            response: Arc::new(Mutex::new(response)),
        }
    }

    pub(crate) fn calls(&self) -> Vec<String> {
        self.calls
            .lock()
            .expect("preferences query calls lock")
            .clone()
    }

    pub(crate) fn set_response(&self, response: UserPreferencesQueryResponse) {
        *self
            .response
            .lock()
            .expect("preferences query response lock") = response;
    }
}

#[async_trait]
impl UserPreferencesQuery for RecordingUserPreferencesQuery {
    async fn fetch_preferences(&self, user_id: &UserId) -> Result<UserPreferences, Error> {
        self.calls
            .lock()
            .expect("preferences query calls lock")
            .push(user_id.to_string());
        match self
            .response
            .lock()
            .expect("preferences query response lock")
            .clone()
        {
            UserPreferencesQueryResponse::Ok(response) => Ok(response),
            UserPreferencesQueryResponse::Err(error) => Err(error),
        }
    }
}
