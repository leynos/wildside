//! Behaviour-driven tests for example data startup seeding.
//!
//! These scenarios validate that the startup seeding flow applies example
//! users once, skips repeat runs and disabled runs, and reports errors for
//! missing seeds or registry failures.
#![cfg(feature = "example-data")]

use rstest_bdd_macros::scenario;
use support::example_data_seeding_world::{ExampleDataSeedingWorld, world};

// NOTE: This binary keeps a handwritten `support` module rather than the
// shared `declare_test_support!` entrypoint. Its Gherkin step definitions live
// in the `example_data_seeding_world` support submodule, and rstest-bdd's
// `#[scenario]` macro resolves steps from a proc-macro-global registry that the
// `#[given]`/`#[when]`/`#[then]` macros must populate *before* the scenarios
// expand. A directly-written `mod support` guarantees that expansion order; a
// `declare_test_support!` bang-macro expansion does not, so routing this
// binary through the shared entrypoint breaks step resolution ("No matching
// step definition found"). The consolidated binaries are unaffected because
// their steps are defined inline rather than in a support submodule.
mod support {
    //! Test-local view of shared support helpers.
    #[path = "../support/mod.rs"]
    mod shared;
    pub use shared::*;
    #[path = "../support/atexit_cleanup.rs"]
    pub mod atexit_cleanup;
    #[path = "../support/cluster_skip.rs"]
    pub mod cluster_skip;
    #[path = "../support/embedded_postgres.rs"]
    pub mod embedded_postgres;
    #[path = "../support/example_data_seeding_world.rs"]
    pub mod example_data_seeding_world;
}

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
