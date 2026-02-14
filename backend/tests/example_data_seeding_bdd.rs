//! Behaviour-driven tests for example data startup seeding.
//!
//! These scenarios validate that the startup seeding flow applies example
//! users once, skips repeat runs and disabled runs, and reports errors for
//! missing seeds or registry failures.
#![cfg(feature = "example-data")]

use rstest_bdd_macros::scenario;
use support::example_data_seeding_world::{ExampleDataSeedingWorld, world};

mod support;

// -----------------------------------------------------------------------------
// Scenario Bindings
// -----------------------------------------------------------------------------

#[scenario(
    path = "tests/features/example_data_seeding.feature",
    name = "First seed run applies example data"
)]
fn first_seed_run_applies_example_data(world: ExampleDataSeedingWorld) {
    drop(world);
}

#[scenario(
    path = "tests/features/example_data_seeding.feature",
    name = "Seed run is skipped when already applied"
)]
fn seed_run_is_skipped_when_already_applied(world: ExampleDataSeedingWorld) {
    drop(world);
}

#[scenario(
    path = "tests/features/example_data_seeding.feature",
    name = "Missing seed returns an error"
)]
fn missing_seed_returns_error(world: ExampleDataSeedingWorld) {
    drop(world);
}

#[scenario(
    path = "tests/features/example_data_seeding.feature",
    name = "Seeding is skipped when disabled"
)]
fn seeding_is_skipped_when_disabled(world: ExampleDataSeedingWorld) {
    drop(world);
}

#[scenario(
    path = "tests/features/example_data_seeding.feature",
    name = "Seeding is skipped when database is missing"
)]
fn seeding_is_skipped_when_database_is_missing(world: ExampleDataSeedingWorld) {
    drop(world);
}

#[scenario(
    path = "tests/features/example_data_seeding.feature",
    name = "Invalid registry path returns an error"
)]
fn invalid_registry_path_returns_error(world: ExampleDataSeedingWorld) {
    drop(world);
}
