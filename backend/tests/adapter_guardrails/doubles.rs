//! Test doubles for driving ports used by the adapter guardrails suite.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use backend::domain::ports::{LoginService, UserOnboarding, UsersQuery};
use backend::domain::{Error, LoginCredentials, User, UserEvent, UserId};
use backend::TraceId;
use uuid::Uuid;

#[derive(Clone)]
pub(crate) enum LoginResponse {
    Ok(UserId),
    Err(Error),
}

#[derive(Clone)]
pub(crate) enum UsersResponse {
    Ok(Vec<User>),
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

#[derive(Clone)]
pub(crate) struct RecordingUsersQuery {
    calls: Arc<Mutex<Vec<String>>>,
    response: Arc<Mutex<UsersResponse>>,
}

impl RecordingUsersQuery {
    pub(crate) fn new(response: UsersResponse) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            response: Arc::new(Mutex::new(response)),
        }
    }

    pub(crate) fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("users calls lock").clone()
    }

    pub(crate) fn set_response(&self, response: UsersResponse) {
        *self.response.lock().expect("users response lock") = response;
    }
}

#[async_trait]
impl UsersQuery for RecordingUsersQuery {
    async fn list_users(&self, authenticated_user: &UserId) -> Result<Vec<User>, Error> {
        self.calls
            .lock()
            .expect("users calls lock")
            .push(authenticated_user.to_string());
        match self.response.lock().expect("users response lock").clone() {
            UsersResponse::Ok(users) => Ok(users),
            UsersResponse::Err(error) => Err(error),
        }
    }
}

#[derive(Clone)]
pub(crate) struct QueueUserOnboarding {
    calls: Arc<Mutex<Vec<(Uuid, String)>>>,
    responses: Arc<Mutex<VecDeque<UserEvent>>>,
}

impl QueueUserOnboarding {
    pub(crate) fn new(responses: Vec<UserEvent>) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            responses: Arc::new(Mutex::new(responses.into())),
        }
    }

    pub(crate) fn calls(&self) -> Vec<(Uuid, String)> {
        self.calls.lock().expect("ws calls lock").clone()
    }

    pub(crate) fn push_response(&self, event: UserEvent) {
        self.responses
            .lock()
            .expect("ws responses lock")
            .push_back(event);
    }
}

impl UserOnboarding for QueueUserOnboarding {
    fn register(&self, trace_id: TraceId, display_name: String) -> UserEvent {
        self.calls
            .lock()
            .expect("ws calls lock")
            .push((*trace_id.as_uuid(), display_name));
        self.responses
            .lock()
            .expect("ws responses lock")
            .pop_front()
            .expect("ws response queue should contain an event")
    }
}
