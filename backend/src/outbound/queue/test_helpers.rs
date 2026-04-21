//! Test utilities for queue adapters.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::{Arc, Mutex};

use crate::domain::ports::JobDispatchError;
use crate::outbound::queue::apalis_route_queue::QueueProvider;

/// Fake queue provider that records pushed payloads for assertion.
///
/// This provider stores all pushed job payloads in-memory, allowing tests to
/// verify that the adapter correctly serializes and pushes jobs without
/// requiring a real PostgreSQL connection.
#[derive(Debug, Clone)]
pub struct FakeQueueProvider {
    /// Shared storage for pushed job payloads.
    pushed_jobs: Arc<Mutex<Vec<Value>>>,
}

impl FakeQueueProvider {
    /// Creates a new fake queue provider.
    pub fn new() -> Self {
        Self {
            pushed_jobs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Returns all job payloads that were pushed to this provider.
    ///
    /// # Errors
    ///
    /// Returns an error if the mutex is poisoned.
    pub fn pushed_jobs(&self) -> Result<Vec<Value>, String> {
        self.pushed_jobs
            .lock()
            .map(|guard| guard.clone())
            .map_err(|e| format!("failed to lock pushed_jobs mutex: {e}"))
    }
}

impl Default for FakeQueueProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl QueueProvider for FakeQueueProvider {
    async fn push_job(&self, payload: Value) -> Result<(), JobDispatchError> {
        self.pushed_jobs
            .lock()
            .map_err(|e| {
                JobDispatchError::unavailable(format!("failed to lock pushed_jobs mutex: {e}"))
            })?
            .push(payload);
        Ok(())
    }
}

/// Fake queue provider that always returns an unavailability error.
///
/// This provider simulates queue unavailability scenarios for testing
/// error handling paths. It always returns `JobDispatchError::Unavailable`.
#[derive(Debug, Clone)]
pub struct FailingQueueProvider {
    error_message: String,
}

impl FailingQueueProvider {
    /// Creates a new failing queue provider with the given error message.
    pub fn new(error_message: String) -> Self {
        Self { error_message }
    }
}

#[async_trait]
impl QueueProvider for FailingQueueProvider {
    async fn push_job(&self, _payload: Value) -> Result<(), JobDispatchError> {
        Err(JobDispatchError::unavailable(self.error_message.clone()))
    }
}
