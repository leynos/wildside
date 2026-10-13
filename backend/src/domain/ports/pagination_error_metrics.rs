//! Domain port for pagination-error metrics and their structured log.

use tracing::info;

use crate::domain::TraceId;

/// Adapter surface that observed a pagination error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaginationErrorSource {
    /// HTTP users-list cursor parsing rejected the request.
    UsersHttp,
    /// User persistence error mapping surfaced a repository pagination error.
    UserPersistence,
}

impl PaginationErrorSource {
    /// Return the stable label used for this adapter surface.
    ///
    /// # Examples
    /// ```
    /// use backend::domain::ports::PaginationErrorSource;
    ///
    /// assert_eq!(PaginationErrorSource::UsersHttp.as_str(), "users_http");
    /// ```
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UsersHttp => "users_http",
            Self::UserPersistence => "user_persistence",
        }
    }
}

/// Metrics recorder for mapped pagination errors.
///
/// The provided [`record`](Self::record) method always emits the structured
/// log before forwarding the observation to the metrics implementation.
pub trait PaginationErrorMetrics: Send + Sync {
    /// Increment the pagination error counter for a bounded detail code.
    ///
    /// Implementations must be synchronous and infallible so metrics never
    /// change request handling.
    ///
    /// # Examples
    /// ```
    /// use backend::domain::ports::{
    ///     NoOpPaginationErrorMetrics, PaginationErrorMetrics, PaginationErrorSource,
    /// };
    ///
    /// NoOpPaginationErrorMetrics.increment(PaginationErrorSource::UsersHttp, "invalid_cursor");
    /// ```
    fn increment(&self, source: PaginationErrorSource, detail_code: &'static str);

    /// Emit the structured mapping log and record the corresponding metric.
    ///
    /// # Examples
    /// ```
    /// use backend::domain::ports::{
    ///     NoOpPaginationErrorMetrics, PaginationErrorMetrics, PaginationErrorSource,
    /// };
    ///
    /// NoOpPaginationErrorMetrics.record(PaginationErrorSource::UsersHttp, "invalid_cursor");
    /// ```
    fn record(&self, source: PaginationErrorSource, detail_code: &'static str) {
        let trace_id = TraceId::current().map(|id| id.to_string());
        info!(
            error_code = detail_code,
            source = source.as_str(),
            trace_id = trace_id.as_deref(),
            "pagination cursor error mapped to client response"
        );
        self.increment(source, detail_code);
    }
}

/// No-op pagination metrics recorder that still emits the structured log.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoOpPaginationErrorMetrics;

impl PaginationErrorMetrics for NoOpPaginationErrorMetrics {
    fn increment(&self, _source: PaginationErrorSource, _detail_code: &'static str) {}
}

#[cfg(test)]
mod tests {
    //! Verify that the no-op recorder preserves structured pagination logs.

    use super::{NoOpPaginationErrorMetrics, PaginationErrorMetrics, PaginationErrorSource};
    use tracing_test::traced_test;

    #[traced_test]
    #[test]
    fn no_op_record_emits_error_code_and_http_source() {
        NoOpPaginationErrorMetrics.record(PaginationErrorSource::UsersHttp, "invalid_cursor");

        assert!(logs_contain("error_code=\"invalid_cursor\""));
        assert!(logs_contain("source=\"users_http\""));
    }

    #[traced_test]
    #[test]
    fn no_op_record_emits_error_code_and_persistence_source() {
        NoOpPaginationErrorMetrics.record(
            PaginationErrorSource::UserPersistence,
            "unsupported_direction",
        );

        assert!(logs_contain("error_code=\"unsupported_direction\""));
        assert!(logs_contain("source=\"user_persistence\""));
    }
}
