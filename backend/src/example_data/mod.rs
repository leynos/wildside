//! Startup wiring for example data seeding.

mod config;
mod startup;

pub use config::ExampleDataSettings;
pub use startup::{StartupSeedingError, seed_example_data_on_startup};
