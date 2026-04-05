//! Test utilities for queue adapters.

#![cfg(test)]

use async_trait::async_trait;
use std::sync::{Arc, Mutex};

use crate::domain::ports::JobDispatchError;
use crate::outbound::queue::apalis_route_queue::QueueProvider;

/// Fake queue provider that records pushed payloads for assertion.
///
/// This provider stores all pushed job payloads in-memory, allowing tests to
/// verify that the adapter correctly serializes and pushes jobs without
/// requiring a real PostgreSQL connection.
#[derive(Debug, Clone)]
pub(crate) struct FakeQueueProvider {
    /// Shared storage for pushed job payloads.
    pushed_jobs: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl FakeQueueProvider {
    /// Creates a new fake queue provider.
    pub(crate) fn new() -> Self {
        Self {
            pushed_jobs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Returns all job payloads that were pushed to this provider.
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    pub(crate) fn pushed_jobs(&self) -> Vec<Vec<u8>> {
        self.pushed_jobs
            .lock()
            .unwrap_or_else(|e| {
                panic!("Mutex poisoned: {e}");
            })
            .clone()
    }
}

#[async_trait]
impl QueueProvider for FakeQueueProvider {
    async fn push_job(&self, payload: Vec<u8>) -> Result<(), JobDispatchError> {
        self.pushed_jobs
            .lock()
            .unwrap_or_else(|e| {
                panic!("Mutex poisoned: {e}");
            })
            .push(payload);
        Ok(())
    }
}

/// Fake queue provider that always returns an error.
///
/// This provider simulates queue unavailability or rejection scenarios for
/// testing error handling paths.
#[derive(Debug, Clone)]
pub(crate) struct FailingQueueProvider {
    error_message: String,
}

impl FailingQueueProvider {
    /// Creates a new failing queue provider with the given error message.
    pub(crate) fn new(error_message: String) -> Self {
        Self { error_message }
    }
}

#[async_trait]
impl QueueProvider for FailingQueueProvider {
    async fn push_job(&self, _payload: Vec<u8>) -> Result<(), JobDispatchError> {
        Err(JobDispatchError::unavailable(self.error_message.clone()))
    }
}
