//! Tests for the Redis route cache adapter.
//!
//! This module contains:
//! - `mock_tests`: Unit tests using in-memory fakes (run unconditionally).
//! - `live_tests`: Integration tests requiring a live `redis-server` binary
//!   (marked with `#[ignore]` and run on-demand).

#[cfg(test)]
mod mock_tests;

#[cfg(test)]
mod live_tests;
