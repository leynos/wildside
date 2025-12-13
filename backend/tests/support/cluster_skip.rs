//! Helpers for controlling embedded test cluster behaviour.
//!
//! Integration tests that depend on embedded PostgreSQL can be optionally
//! skipped in environments where the cluster cannot be started. This module
//! centralises the `SKIP_TEST_CLUSTER` policy and error messaging so all test
//! suites behave consistently.

/// Returns true when the `SKIP_TEST_CLUSTER` environment variable is set to a
/// truthy value.
///
/// Truthy values: "1", "true", "yes" (case-insensitive).
pub fn should_skip_test_cluster() -> bool {
    std::env::var("SKIP_TEST_CLUSTER")
        .map(|value| matches!(value.to_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

/// Handles embedded cluster setup failures consistently across integration tests.
///
/// When `SKIP_TEST_CLUSTER` is truthy, prints a skip marker and returns `None`.
/// Otherwise, panics with a clear failure message so CI breakage is not masked.
pub fn handle_cluster_setup_failure<T>(reason: impl std::fmt::Display) -> Option<T> {
    if should_skip_test_cluster() {
        eprintln!("SKIP-TEST-CLUSTER: {reason}");
        None
    } else {
        panic!("Test cluster setup failed: {reason}. Set SKIP_TEST_CLUSTER=1 to skip.");
    }
}
