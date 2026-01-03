//! Test doubles for user-related driving ports.

use std::sync::{Arc, Mutex};

use super::recording_double_macro::recording_double;
use async_trait::async_trait;
use backend::domain::ports::{LoginService, UserInterestsCommand, UserProfileQuery, UsersQuery};
use backend::domain::{Error, InterestThemeId, LoginCredentials, User, UserId, UserInterests};

/// Configurable success or failure outcome for RecordingLoginService.
#[derive(Clone)]
pub(crate) enum LoginResponse {
    Ok(UserId),
    Err(Error),
}

/// Configurable success or failure outcome for RecordingUserInterestsCommand.
#[derive(Clone)]
pub(crate) enum UserInterestsResponse {
    Ok(UserInterests),
    Err(Error),
}

#[derive(Clone)]
pub(crate) struct RecordingLoginService {
    calls: Arc<Mutex<Vec<(String, String)>>>,
    response: Arc<Mutex<LoginResponse>>,
}

impl RecordingLoginService {
    pub(crate) fn new(response: LoginResponse) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            response: Arc::new(Mutex::new(response)),
        }
    }

    pub(crate) fn calls(&self) -> Vec<(String, String)> {
        self.calls.lock().expect("login calls lock").clone()
    }

    pub(crate) fn set_response(&self, response: LoginResponse) {
        *self.response.lock().expect("login response lock") = response;
    }
}

#[async_trait]
impl LoginService for RecordingLoginService {
    async fn authenticate(&self, credentials: &LoginCredentials) -> Result<UserId, Error> {
        self.calls.lock().expect("login calls lock").push((
            credentials.username().to_owned(),
            credentials.password().to_owned(),
        ));
        match self.response.lock().expect("login response lock").clone() {
            LoginResponse::Ok(user_id) => Ok(user_id),
            LoginResponse::Err(error) => Err(error),
        }
    }
}

recording_double! {
    /// Configurable success or failure outcome for RecordingUsersQuery.
    pub(crate) enum UsersResponse {
        Ok(Vec<User>),
        Err(Error),
    }

    pub(crate) struct RecordingUsersQuery {
        calls: String,
        trait: UsersQuery,
        method: list_users(&self, authenticated_user: &UserId) -> Result<Vec<User>, Error>,
        record: authenticated_user.to_string(),
        calls_lock: "users calls lock",
        response_lock: "users response lock",
    }
}

recording_double! {
    /// Configurable success or failure outcome for RecordingUserProfileQuery.
    pub(crate) enum UserProfileResponse {
        Ok(User),
        Err(Error),
    }

    pub(crate) struct RecordingUserProfileQuery {
        calls: String,
        trait: UserProfileQuery,
        method: fetch_profile(&self, user_id: &UserId) -> Result<User, Error>,
        record: user_id.to_string(),
        calls_lock: "profile calls lock",
        response_lock: "profile response lock",
    }
}

#[derive(Clone)]
pub(crate) struct RecordingUserInterestsCommand {
    calls: Arc<Mutex<Vec<(String, Vec<String>)>>>,
    response: Arc<Mutex<UserInterestsResponse>>,
}

impl RecordingUserInterestsCommand {
    pub(crate) fn new(response: UserInterestsResponse) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            response: Arc::new(Mutex::new(response)),
        }
    }

    pub(crate) fn calls(&self) -> Vec<(String, Vec<String>)> {
        self.calls.lock().expect("interests calls lock").clone()
    }

    pub(crate) fn set_response(&self, response: UserInterestsResponse) {
        *self.response.lock().expect("interests response lock") = response;
    }
}

#[async_trait]
impl UserInterestsCommand for RecordingUserInterestsCommand {
    async fn set_interests(
        &self,
        user_id: &UserId,
        interest_theme_ids: Vec<InterestThemeId>,
    ) -> Result<UserInterests, Error> {
        self.calls.lock().expect("interests calls lock").push((
            user_id.to_string(),
            interest_theme_ids.iter().map(|id| id.to_string()).collect(),
        ));
        match self
            .response
            .lock()
            .expect("interests response lock")
            .clone()
        {
            UserInterestsResponse::Ok(interests) => Ok(interests),
            UserInterestsResponse::Err(error) => Err(error),
        }
    }
}
