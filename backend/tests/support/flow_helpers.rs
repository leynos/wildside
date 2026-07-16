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
/// # Parameters
///
/// - `bytes`: the raw response body.
///
/// # Returns
///
/// `None` when `bytes` is empty, otherwise `Some(value)` with the parsed JSON.
///
/// # Panics
///
/// Panics if a non-empty `bytes` is not valid JSON.
///
/// # Examples
///
/// ```no_run
/// # use serde_json::{json, Value};
/// # fn parse_json_body(bytes: &[u8]) -> Option<Value> {
/// #     (!bytes.is_empty()).then(|| serde_json::from_slice(bytes).expect("json body"))
/// # }
/// // An empty body (e.g. a 204 response) is reported as absent.
/// assert_eq!(parse_json_body(b""), None);
/// // A populated body is parsed into a JSON value.
/// assert_eq!(
///     parse_json_body(br#"{"status":"ok"}"#),
///     Some(json!({ "status": "ok" })),
/// );
/// ```
pub fn parse_json_body(bytes: &[u8]) -> Option<Value> {
    (!bytes.is_empty()).then(|| serde_json::from_slice(bytes).expect("json body"))
}
