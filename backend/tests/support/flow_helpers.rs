//! Async-runtime and JSON-body helpers shared by BDD flow tests.
//!
//! These are the small, behaviour-identical helpers that several `*_bdd`
//! flow modules previously duplicated. Sharing them keeps a single source of
//! truth while leaving scenario-specific wiring (HTTP state construction,
//! fixture matching, fixture constants) local to each test binary.

use std::future::Future;

use serde_json::Value;

/// Drives an async future to completion on a fresh single-threaded runtime.
///
/// A new current-thread Tokio runtime (with all drivers enabled) is created
/// per call so each step runs in isolation without sharing an executor. The
/// current-thread flavour keeps blocking BDD steps deterministic and avoids
/// spawning worker threads per scenario.
///
/// # Parameters
///
/// - `future`: the future to run to completion.
///
/// # Returns
///
/// The value the future resolves to.
///
/// # Examples
///
/// ```no_run
/// # fn run_async<T>(future: impl std::future::Future<Output = T>) -> T {
/// #     tokio::runtime::Builder::new_current_thread()
/// #         .enable_all()
/// #         .build()
/// #         .expect("runtime")
/// #         .block_on(future)
/// # }
/// // `run_async` drives the future and returns its resolved value.
/// let doubled = run_async(async { 21 * 2 });
/// assert_eq!(doubled, 42);
/// ```
pub fn run_async<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(future)
}

/// Parses an HTTP response body as JSON, treating an empty body as absent.
///
/// Parsing failure is returned rather than hidden behind a panic, so each
/// caller decides how to surface a malformed body at its response-capture
/// boundary.
///
/// # Parameters
///
/// - `bytes`: the raw response body.
///
/// # Returns
///
/// - `Ok(None)` when `bytes` is empty (for example a `204 No Content`).
/// - `Ok(Some(value))` when `bytes` is non-empty and valid JSON.
/// - `Err(serde_json::Error)` when `bytes` is non-empty but not valid JSON.
///
/// # Examples
///
/// ```no_run
/// # use serde_json::{json, Value};
/// # fn parse_json_body(bytes: &[u8]) -> Result<Option<Value>, serde_json::Error> {
/// #     if bytes.is_empty() { Ok(None) } else { serde_json::from_slice(bytes).map(Some) }
/// # }
/// // An empty body (e.g. a 204 response) is reported as absent.
/// assert_eq!(parse_json_body(b"").unwrap(), None);
/// // A populated body is parsed into a JSON value.
/// assert_eq!(
///     parse_json_body(br#"{"status":"ok"}"#).unwrap(),
///     Some(json!({ "status": "ok" })),
/// );
/// // Malformed JSON surfaces as an error instead of a panic.
/// assert!(parse_json_body(b"{ not json").is_err());
/// ```
pub fn parse_json_body(bytes: &[u8]) -> Result<Option<Value>, serde_json::Error> {
    if bytes.is_empty() {
        Ok(None)
    } else {
        serde_json::from_slice(bytes).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_json_body_returns_none_for_empty_input() {
        assert_eq!(parse_json_body(b"").expect("empty body must be Ok"), None);
    }

    #[test]
    fn parse_json_body_parses_valid_json() {
        assert_eq!(
            parse_json_body(br#"{"status":"ok"}"#).expect("valid json must be Ok"),
            Some(serde_json::json!({ "status": "ok" })),
        );
    }

    #[test]
    fn parse_json_body_reports_malformed_json_as_error() {
        assert!(
            parse_json_body(b"{ not valid json").is_err(),
            "malformed non-empty JSON must be an error, not a panic",
        );
    }

    #[test]
    fn run_async_drives_future_to_completion() {
        assert_eq!(run_async(async { 21 * 2 }), 42);
    }
}
