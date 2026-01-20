//! Deterministic example user data generation for demonstration purposes.
//!
//! This crate provides tools for generating believable, reproducible user data
//! from a JSON seed registry. It is designed to be independent of backend
//! domain types to avoid circular dependencies.
//!
//! # Overview
//!
//! The crate supports:
//!
//! - Loading seed registries from JSON files
//! - Deterministic user generation using named seeds
//! - Display name validation matching backend constraints
//! - Configurable interest themes and safety toggles
//!
//! # Example
//!
//! ```
//! use example_data::{SeedRegistry, generate_example_users};
//!
//! let json = r#"{
//!     "version": 1,
//!     "interestThemeIds": ["3fa85f64-5717-4562-b3fc-2c963f66afa6"],
//!     "safetyToggleIds": ["7fa85f64-5717-4562-b3fc-2c963f66afa6"],
//!     "seeds": [{"name": "test-seed", "seed": 42, "userCount": 3}]
//! }"#;
//!
//! let registry = SeedRegistry::from_json(json).expect("valid registry");
//! let seed_def = registry.find_seed("test-seed").expect("seed exists");
//! let users = generate_example_users(&registry, seed_def).expect("generation succeeds");
//!
//! assert_eq!(users.len(), 3);
//! ```

mod atomic_io;
mod error;
mod generator;
mod registry;
mod seed;
pub mod seed_registry_cli;
mod validation;

pub use error::{GenerationError, RegistryError};
pub use generator::generate_example_users;
pub use registry::{SeedDefinition, SeedRegistry};
pub use seed::{ExampleUserSeed, UnitSystemSeed};
pub use validation::{DISPLAY_NAME_MAX, DISPLAY_NAME_MIN, is_valid_display_name};
