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

mod diesel_basic_error_mapping;
mod diesel_catalogue_ingestion_repository;
mod diesel_catalogue_repository;
mod diesel_descriptor_ingestion_repository;
mod diesel_descriptor_repository;
mod diesel_example_data_runs_repository;
mod diesel_example_data_seed_repository;
pub(crate) mod diesel_helpers;
mod diesel_idempotency_repository;
mod diesel_login_service;
mod diesel_offline_bundle_repository;
mod diesel_osm_ingestion_provenance_repository;
mod diesel_osm_poi_repository;
mod diesel_route_annotation_repository;
mod diesel_user_preferences_repository;
mod diesel_user_repository;
mod diesel_users_query;
mod diesel_walk_session_repository;
mod ingestion_upsert_macros;
mod json_serializers;
mod models;
mod pool;
mod postgres_schema_snapshot_repository;
mod schema;
mod user_persistence_error_mapping;

pub use diesel_catalogue_ingestion_repository::DieselCatalogueIngestionRepository;
pub use diesel_catalogue_repository::DieselCatalogueRepository;
pub use diesel_descriptor_ingestion_repository::DieselDescriptorIngestionRepository;
pub use diesel_descriptor_repository::DieselDescriptorRepository;
pub use diesel_example_data_runs_repository::DieselExampleDataRunsRepository;
pub use diesel_example_data_seed_repository::DieselExampleDataSeedRepository;
pub use diesel_idempotency_repository::DieselIdempotencyRepository;
pub use diesel_login_service::DieselLoginService;
pub use diesel_offline_bundle_repository::DieselOfflineBundleRepository;
pub use diesel_osm_ingestion_provenance_repository::DieselOsmIngestionProvenanceRepository;
pub use diesel_osm_poi_repository::DieselOsmPoiRepository;
pub use diesel_route_annotation_repository::DieselRouteAnnotationRepository;
pub use diesel_user_preferences_repository::DieselUserPreferencesRepository;
pub use diesel_user_repository::DieselUserRepository;
pub use diesel_users_query::DieselUsersQuery;
pub use diesel_walk_session_repository::DieselWalkSessionRepository;
pub use pool::{DbPool, PoolConfig, PoolError};
pub use postgres_schema_snapshot_repository::PostgresSchemaSnapshotRepository;
