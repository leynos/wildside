//! Redis protocol helpers for integration tests.
//!
//! Re-exports the shared `RedisTestServer` from the backend crate's test-support module.

pub type RedisTestServer = backend::test_support::redis::RedisTestServer;
