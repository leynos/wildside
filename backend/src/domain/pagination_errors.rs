//! Shared pagination error envelope constructors.
//!
//! These helpers centralize the user-visible cursor error contract so inbound
//! adapters and repository error mapping cannot drift.
//!
//! * `invalid_cursor_error_from` — pure constructor for the invalid-cursor
//!   error envelope.
//! * `unsupported_direction_error_from` — pure constructor for the
//!   unsupported-direction error envelope.
//! * `record_pagination_error` — emit structured log and increment counter.

use serde_json::json;
#[cfg(feature = "metrics")]
use std::sync::OnceLock;
use tracing::info;

use super::{Error, TraceId};

#[cfg(feature = "metrics")]
use prometheus::{IntCounterVec, Opts, Registry};

#[cfg(feature = "metrics")]
static PAGINATION_ERRORS_TOTAL: OnceLock<IntCounterVec> = OnceLock::new();

/// Adapter surface that observed a pagination error.
#[derive(Debug, Clone, Copy)]
pub(crate) enum PaginationErrorSource {
    /// HTTP users-list cursor parsing rejected the request.
    UsersHttp,
    /// User persistence error mapping surfaced a repository pagination error.
    UserPersistence,
}

impl PaginationErrorSource {
    const fn as_str(self) -> &'static str {
        match self {
            Self::UsersHttp => "users_http",
            Self::UserPersistence => "user_persistence",
        }
    }
}

/// Build the standard invalid-cursor error envelope.
pub(crate) fn invalid_cursor_error_from(_source: PaginationErrorSource) -> Error {
    Error::invalid_request("cursor is invalid")
        .with_details(json!({ "field": "cursor", "code": "invalid_cursor" }))
}

/// Build the standard unsupported-direction error envelope.
pub(crate) fn unsupported_direction_error_from(_source: PaginationErrorSource) -> Error {
    Error::invalid_request("cursor direction is unsupported")
        .with_details(json!({ "field": "cursor", "code": "unsupported_direction" }))
}

/// Record a pagination cursor error observation: structured log + optional counter.
pub(crate) fn record_pagination_error(source: PaginationErrorSource, detail_code: &'static str) {
    let trace_id = TraceId::current().map(|id| id.to_string());
    info!(
        error_code = detail_code,
        source = source.as_str(),
        trace_id = trace_id.as_deref(),
        "pagination cursor error mapped to client response"
    );
    increment_pagination_error_counter(source, detail_code);
}

#[cfg(feature = "metrics")]
fn increment_pagination_error_counter(source: PaginationErrorSource, detail_code: &str) {
    if let Some(counter) = PAGINATION_ERRORS_TOTAL.get() {
        counter
            .with_label_values(&[detail_code, source.as_str()])
            .inc();
    }
}

#[cfg(not(feature = "metrics"))]
const fn increment_pagination_error_counter(_source: PaginationErrorSource, _detail_code: &str) {}

/// Register pagination error counters on the Prometheus registry.
#[cfg(feature = "metrics")]
pub(crate) fn register_pagination_error_metrics(
    registry: &Registry,
) -> Result<(), prometheus::Error> {
    let counter = IntCounterVec::new(
        Opts::new(
            "wildside_pagination_errors_total",
            "Total pagination cursor errors mapped to client responses",
        ),
        &["code", "source"],
    )?;
    registry.register(Box::new(counter.clone()))?;
    let _ = PAGINATION_ERRORS_TOTAL.set(counter);
    Ok(())
}

#[cfg(all(test, feature = "metrics"))]
mod tests {
    //! Regression coverage for pagination error metrics registration.

    use std::error::Error as StdError;

    use serial_test::serial;

    use super::*;

    type TestResult<T = ()> = Result<T, Box<dyn StdError>>;

    #[test]
    #[serial]
    fn registers_pagination_error_counter() -> TestResult {
        let registry = Registry::new();

        register_pagination_error_metrics(&registry)?;
        record_pagination_error(PaginationErrorSource::UsersHttp, "invalid_cursor");

        let families = registry.gather();
        let counter_value = families
            .iter()
            .find(|metric| metric.name() == "wildside_pagination_errors_total")
            .and_then(|metric| metric.metric.first())
            .and_then(|sample| sample.counter.as_ref())
            .map(|counter| counter.value());

        assert_eq!(
            counter_value,
            Some(1.0),
            "record_pagination_error should increment wildside_pagination_errors_total"
        );
        Ok(())
    }
}
