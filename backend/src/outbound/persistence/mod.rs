//! PostgreSQL persistence adapters using Diesel ORM.
//!
//! This module provides concrete implementations of domain repository ports
//! backed by PostgreSQL via the Diesel ORM with async support through
//! `diesel-async` and `bb8` connection pooling.
//!
//! # Architecture
//!
//! The persistence layer follows these principles:
//!
//! - **Thin adapters**: Repository implementations only translate between
//!   Diesel models and domain types. No business logic resides here.
//! - **Internal models**: Diesel row structs (`models.rs`) and schema
//!   definitions (`schema.rs`) are internal implementation details, never
//!   exposed to the domain layer.
//! - **Async-safe pooling**: Connections are managed via `bb8` pools with
//!   proper async integration through `diesel-async`.
//! - **Strongly typed errors**: All database errors are mapped to domain
//!   persistence error types.
//!
//! # Example
//!
//! ```ignore
//! use backend::outbound::persistence::{DbPool, PoolConfig, DieselUserRepository};
//!
//! let config = PoolConfig::new("postgres://localhost/mydb");
//! let pool = DbPool::new(config).await?;
//! let repo = DieselUserRepository::new(pool);
//! ```

pub(crate) mod diesel_helpers;
mod diesel_idempotency_repository;
mod diesel_route_annotation_repository;
mod diesel_user_preferences_repository;
mod diesel_user_repository;
mod models;
mod pool;
mod schema;

pub use diesel_idempotency_repository::DieselIdempotencyRepository;
pub use diesel_route_annotation_repository::DieselRouteAnnotationRepository;
pub use diesel_user_preferences_repository::DieselUserPreferencesRepository;
pub use diesel_user_repository::DieselUserRepository;
pub use pool::{DbPool, PoolConfig, PoolError};
