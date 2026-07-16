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
/// A new Tokio runtime is created per call so each step runs in isolation
/// without sharing an executor.
///
/// # Parameters
///
/// - `future`: the future to run to completion.
///
/// # Returns
///
/// The value the future resolves to.
pub fn run_async<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Runtime::new()
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
pub fn parse_json_body(bytes: &[u8]) -> Option<Value> {
    (!bytes.is_empty()).then(|| serde_json::from_slice(bytes).expect("json body"))
}
