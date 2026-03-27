//! Redis protocol helpers for integration tests.
//!
//! Re-exports the shared `RedisTestServer` from the backend crate's test-support module.

#[allow(
    unused_imports,
    reason = "re-exported for integration tests that need Redis"
)]
pub use backend::test_support::redis::RedisTestServer;
