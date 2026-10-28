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
//! The Apalis adapter uses an internal provider abstraction (`QueueProvider` trait)
//! to enable unit testing without PostgreSQL, following the same pattern as
//! [`RedisRouteCache`](crate::outbound::cache::RedisRouteCache).
//!
//! Persisted payloads must pass through [`decode_job`] before a worker handler
//! receives them. This keeps version rejection and domain validation at the
//! queue boundary rather than duplicating it in each handler.
//!
//! # Delivery scope
//!
//! Durable retry scheduling and dead-letter operations are implemented here.
//! Worker deployment and request-path dispatch remain roadmap work, as does
//! trace ID propagation through job metadata.
//!
//! See `docs/backend-roadmap.md` section 5.2 for the implementation roadmap.

mod stub_route_queue;
pub use stub_route_queue::StubRouteQueue;

mod apalis_route_queue;
pub use apalis_route_queue::{ApalisPostgresProvider, ApalisRouteQueue, GenericApalisRouteQueue};

mod postgres_dead_letter;
pub use postgres_dead_letter::{PostgresDeadLetterQueue, RequeueBudgets};

mod postgres_retry_scheduler;
pub use postgres_retry_scheduler::{PostgresRetryScheduler, TerminalRetryReason};

mod retrying_handler;
pub use retrying_handler::{RetryingJobService, RetryingJobServiceConfig};

mod job_decode;
pub use job_decode::decode_job;

#[cfg(any(test, feature = "test-support"))]
pub mod test_helpers;
