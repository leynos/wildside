//! Outbound adapters implementing domain ports for external infrastructure.
//!
//! This module follows the hexagonal architecture pattern, providing concrete
//! implementations of domain port traits for various infrastructure concerns:
//!
//! - **persistence**: PostgreSQL-backed repositories using Diesel ORM
//! - **cache**: Redis-backed caching (stub implementation pending)
//! - **queue**: Apalis-backed job queue (stub implementation pending)
//! - **metrics**: Prometheus-backed metrics exporters (feature-gated)
//!
//! Adapters are thin translators that convert between domain types and
//! infrastructure-specific representations. They contain no business logic.

pub mod cache;
#[cfg(feature = "metrics")]
pub mod metrics;
pub mod osm_source;
pub mod persistence;
pub mod queue;
