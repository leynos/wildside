//! Driving port for WebSocket onboarding commands.
//!
//! WebSocket inbound adapters should remain responsible for framing and
//! transport details, while domain behaviour (validation, event emission)
//! lives behind this port. A synchronous interface is intentional: onboarding
//! is currently CPU-only and must not perform I/O.

use crate::TraceId;
use crate::domain::UserEvent;

/// Domain use-case port for user onboarding.
pub trait UserOnboarding: Send + Sync {
    /// Validate a display name and emit a domain event.
    fn register(&self, trace_id: TraceId, display_name: String) -> UserEvent;
}
