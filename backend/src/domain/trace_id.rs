//! Request-scoped trace identifier for correlation across logs and errors.
//!
//! `TraceId` is a domain primitive representing a correlation identifier that
//! follows a request through the system. It uses task-local storage to make the
//! current trace identifier available without explicit parameter threading.
//!
//! Tokio task-local variables are not inherited across spawned tasks. Use
//! [`TraceId::scope`] when spawning new tasks or moving work onto blocking
//! threads to ensure the active trace identifier propagates correctly.

use std::future::Future;

use tokio::task_local;
use uuid::Uuid;

task_local! {
    /// Task-local storage for the current trace identifier.
    pub(crate) static TRACE_ID: TraceId;
}

/// Per-request trace identifier exposed via task-local storage.
///
/// # Examples
/// ```
/// use backend::TraceId;
///
/// async fn handler() {
///     if let Some(id) = TraceId::current() {
///         println!("trace id: {}", id);
///     }
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceId(pub(crate) Uuid);

impl TraceId {
    /// Generate a new random trace identifier.
    #[must_use]
    #[rustfmt::skip]
    pub(crate) fn generate() -> Self { Self(Uuid::new_v4()) }

    /// Construct a trace identifier from an existing UUID.
    #[must_use]
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Returns the current trace identifier if one is in scope.
    #[must_use]
    #[rustfmt::skip]
    pub fn current() -> Option<Self> { TRACE_ID.try_with(|id| *id).ok() }

    /// Access the inner UUID.
    #[must_use]
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Execute the provided future with the supplied trace identifier in scope.
    ///
    /// # Examples
    /// ```
    /// use backend::TraceId;
    ///
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// let trace_id: TraceId = "00000000-0000-0000-0000-000000000000"
    ///     .parse()
    ///     .expect("valid UUID");
    /// let observed = TraceId::scope(trace_id, async move { TraceId::current() }).await;
    /// assert_eq!(observed, Some(trace_id));
    /// # });
    /// ```
    pub async fn scope<Fut>(trace_id: TraceId, fut: Fut) -> Fut::Output
    where
        Fut: Future,
    {
        TRACE_ID.scope(trace_id, fut).await
    }
}

impl std::fmt::Display for TraceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for TraceId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for this module.
    use super::*;

    #[tokio::test]
    async fn generate_produces_uuid() {
        let trace_id = TraceId::generate();
        let parsed = Uuid::parse_str(&trace_id.to_string()).expect("valid UUID");
        assert_eq!(parsed.to_string(), trace_id.to_string());
    }

    #[tokio::test]
    async fn current_reflects_scope() {
        let expected = TraceId::generate();
        let observed = TraceId::scope(expected, async move { TraceId::current() }).await;
        assert_eq!(observed, Some(expected));
    }

    #[tokio::test]
    async fn current_is_none_out_of_scope() {
        assert!(TraceId::current().is_none());
    }

    #[tokio::test]
    async fn from_str_round_trips() {
        let uuid = Uuid::nil();
        let trace_id: TraceId = uuid.to_string().parse().expect("parse uuid");
        assert_eq!(trace_id.to_string(), uuid.to_string());
    }

    #[test]
    fn from_uuid_round_trips() {
        let uuid = Uuid::new_v4();
        let trace_id = TraceId::from_uuid(uuid);
        assert_eq!(trace_id.as_uuid(), &uuid);
    }

    #[tokio::test]
    async fn from_uuid_preserves_current_scope() {
        let uuid = Uuid::new_v4();
        let trace_id = TraceId::from_uuid(uuid);
        let observed = TraceId::scope(trace_id, async { TraceId::current() }).await;
        assert_eq!(observed, Some(trace_id));
    }
}
