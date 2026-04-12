//! Queue adapters for the `RouteQueue` port.
//!
//! This module provides implementations of the `RouteQueue` port:
//!
//! - [`StubRouteQueue`]: A no-op stub that discards all jobs (for development)
//! - [`ApalisRouteQueue`]: Production adapter using Apalis with PostgreSQL storage
//!
//! # Architecture
//!
//! The queue adapters follow the hexagonal architecture pattern, implementing
//! the domain-owned [`RouteQueue`](crate::domain::ports::RouteQueue) port with
//! infrastructure-specific details isolated in this module.
//!
//! The Apalis adapter uses a provider abstraction (internal `QueueProvider` trait)
//! to enable unit testing without PostgreSQL, following the same pattern as
//! [`RedisRouteCache`](crate::outbound::cache::RedisRouteCache).
//!
//! # Future Implementation
//!
//! The full queue system will include:
//! - Worker consumption of enqueued jobs
//! - Job struct definitions (`GenerateRouteJob`, `EnrichmentJob`)
//! - Retry policies with exponential backoff
//! - Dead-letter handling for failed jobs
//! - Trace ID propagation through job metadata
//!
//! See `docs/backend-roadmap.md` section 5.2 for the implementation roadmap.

mod stub_route_queue;
pub use stub_route_queue::StubRouteQueue;

mod apalis_route_queue;
pub use apalis_route_queue::{
    ApalisPostgresProvider, ApalisRouteQueue, GenericApalisRouteQueue, QueueProvider,
};

#[cfg(test)]
mod test_helpers;
