//! Behavioural tests for offline bundle and walk session repositories.
use rstest::fixture;
use rstest_bdd_macros::scenario;
use std::sync::{Arc, Mutex};

mod offline_bundle_walk_session_bdd {
    //! Split helpers for offline-bundle and walk-session behavioural tests.
    pub mod contract_checks;
    pub mod offline_bundle_steps;
    pub mod repository_impl;
    pub mod steps_helpers;
    pub mod test_data;
    pub mod walk_session_steps;
}
mod support {
    //! Test-local view of shared support helpers.
    include!("support/mod.rs");
    #[path = "../support/atexit_cleanup.rs"]
    pub mod atexit_cleanup;
    #[path = "../support/cluster_skip.rs"]
    pub mod cluster_skip;
    #[path = "../support/embedded_postgres.rs"]
    pub mod embedded_postgres;
    #[path = "../support/seed_helpers.rs"]
    pub mod seed_helpers;
    #[path = "../support/table_helpers.rs"]
    pub mod table_helpers;
}

use offline_bundle_walk_session_bdd::steps_helpers::{SharedContext, setup_test_context};
use support::cluster_skip::handle_cluster_setup_failure;

#[fixture]
fn world() -> SharedContext {
    match setup_test_context() {
        Ok(ctx) => Arc::new(Mutex::new(ctx)),
        Err(reason) => {
            let _: Option<()> = handle_cluster_setup_failure(reason);
            panic!("SKIP-TEST-CLUSTER");
        }
    }
}

#[scenario(
    path = "tests/features/offline_bundle_walk_session.feature",
    name = "Repositories persist manifests and completion summaries with query-error mapping"
)]
fn repositories_persist_manifests_and_completion_summaries_with_query_error_mapping(
    world: SharedContext,
) {
    drop(world);
}
