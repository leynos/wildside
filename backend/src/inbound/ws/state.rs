//! Shared WebSocket adapter state.
//!
//! WebSocket entry points should depend on domain ports (use-cases) instead of
//! constructing domain services directly. This makes the adapter testable with
//! deterministic test doubles and keeps side effects out of the actor.

use std::sync::Arc;

use crate::domain::ports::UserOnboarding;

/// Dependency bundle for WebSocket handlers and actors.
#[derive(Clone)]
pub struct WsState {
    pub onboarding: Arc<dyn UserOnboarding>,
}

impl WsState {
    /// Construct state from explicit port implementations.
    pub fn new(onboarding: Arc<dyn UserOnboarding>) -> Self {
        Self { onboarding }
    }
}
