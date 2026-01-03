//! Test doubles for WebSocket onboarding.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use backend::TraceId;
use backend::domain::UserEvent;
use backend::domain::ports::UserOnboarding;
use uuid::Uuid;

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
