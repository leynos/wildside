//! WebSocket-focused test helpers.
//!
//! Integration tests under `backend/tests/` compile as separate crates, so
//! sharing small WebSocket setup helpers helps avoid copy/paste drift.

use std::sync::Arc;

use backend::domain::ports::UserOnboarding;
use backend::inbound::ws::state::WsState;

/// Build a `WsState` for use in tests.
///
/// This helper hides the repetitive `WsState::new(Arc::new(...))` boilerplate
/// and keeps setup consistent across integration test crates.
pub fn ws_state<T>(onboarding: T) -> WsState
where
    T: UserOnboarding + 'static,
{
    WsState::new(Arc::new(onboarding))
}
