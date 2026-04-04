//! Behaviour coverage for 3.5.5 comprehensive startup-mode composition.
//!
//! This BDD suite exercises all 16 HTTP-facing ports across fixture-fallback
//! and DB-present startup modes, proving that adapter selection remains
//! deterministic at the HTTP boundary with embedded PostgreSQL backing.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use uuid::Uuid;

mod support;

use support::embedded_postgres::drop_users_table;
use support::profile_interests::FIXTURE_AUTH_ID;
use support::handle_cluster_setup_failure;

#[path = "../src/server/config.rs"]
#[expect(
    dead_code,
    reason = "tests import ServerConfig from server_config for BDD startup-mode checks"
)]
mod server_config;
pub(crate) use server_config::ServerConfig;

#[path = "../src/server/state_builders.rs"]
mod state_builders;

#[path = "startup_mode_composition_bdd/flow_support.rs"]
mod flow_support;

use flow_support::{
    World, assert_internal, assert_profile_response, is_skipped, run_comprehensive_flow,
    run_validation_error_flow, seed_user, setup_db_context,
};

const DB_PROFILE_NAME: &str = "Test User DB";
const FIXTURE_PROFILE_NAME: &str = "Ada Lovelace";

#[fixture]
fn world() -> World {
    World {
        db: None,
        login: None,
        profile: None,
        interests: None,
        preferences: None,
        catalogue_explore: None,
        catalogue_descriptors: None,
        offline_bundles: None,
        walk_sessions: None,
        enrichment_provenance: None,
        skip_reason: None,
    }
}

// ------------------------------------------------------------------------
// Scenario 1: Fixture-fallback happy path
// ------------------------------------------------------------------------

#[given("fixture-fallback startup mode without a database pool")]
fn fixture_fallback_startup_mode_without_a_database_pool(world: &mut World) {
    world.db = None;
    world.skip_reason = None;
}

#[when("executing requests against all major endpoint groups")]
fn executing_requests_against_all_major_endpoint_groups(world: &mut World) {
    run_comprehensive_flow(world);
}

#[then("all responses match fixture fallback contracts")]
fn all_responses_match_fixture_fallback_contracts(world: &mut World) {
    if is_skipped(world) {
        return;
    }

    // Login should succeed with fixture credentials
    let login = world.login.as_ref().expect("login snapshot");
    assert_eq!(login.status, 200);
    assert!(login.session_cookie.is_some());

    // Profile should return fixture display name
    let profile = world.profile.as_ref().expect("profile snapshot");
    assert_profile_response(profile, FIXTURE_PROFILE_NAME);

    // Preferences should return 200 with fixture data
    let preferences = world.preferences.as_ref().expect("preferences snapshot");
    assert_eq!(preferences.status, 200);
    let prefs_body = preferences.body.as_ref().expect("preferences body");
    assert!(prefs_body.get("userId").is_some());
    assert!(prefs_body.get("interestThemeIds").is_some());
    assert!(prefs_body.get("safetyToggleIds").is_some());
    assert!(prefs_body.get("unitSystem").is_some());
    assert!(prefs_body.get("revision").is_some());

    // Catalogue explore should return 200 with fixture data
    let catalogue_explore = world
        .catalogue_explore
        .as_ref()
        .expect("catalogue_explore snapshot");
    assert_eq!(catalogue_explore.status, 200);
    let explore_body = catalogue_explore
        .body
        .as_ref()
        .expect("catalogue_explore body");
    assert!(explore_body.get("generatedAt").is_some());
    assert!(explore_body.get("categories").is_some());
    assert!(explore_body.get("routes").is_some());

    // Catalogue descriptors should return 200 with fixture data
    let catalogue_descriptors = world
        .catalogue_descriptors
        .as_ref()
        .expect("catalogue_descriptors snapshot");
    assert_eq!(catalogue_descriptors.status, 200);
    let descriptors_body = catalogue_descriptors
        .body
        .as_ref()
        .expect("catalogue_descriptors body");
    assert!(descriptors_body.get("generatedAt").is_some());
    assert!(descriptors_body.get("tags").is_some());
    assert!(descriptors_body.get("badges").is_some());
    assert!(descriptors_body.get("interestThemes").is_some());

    // Offline bundles should return 200 with empty fixture list
    let offline_bundles = world
        .offline_bundles
        .as_ref()
        .expect("offline_bundles snapshot");
    assert_eq!(offline_bundles.status, 200);
    let bundles_body = offline_bundles
        .body
        .as_ref()
        .expect("offline_bundles body");
    assert!(bundles_body.get("bundles").is_some());

    // Enrichment provenance should return 200 with empty fixture list
    let enrichment = world
        .enrichment_provenance
        .as_ref()
        .expect("enrichment_provenance snapshot");
    assert_eq!(enrichment.status, 200);
    let enrichment_body = enrichment
        .body
        .as_ref()
        .expect("enrichment_provenance body");
    assert!(enrichment_body.get("records").is_some());
}

// ------------------------------------------------------------------------
// Scenario 2: DB-present happy path
// ------------------------------------------------------------------------

#[given("db-present startup mode backed by embedded postgres")]
fn db_present_startup_mode_backed_by_embedded_postgres(world: &mut World) {
    match setup_db_context() {
        Ok(db) => {
            match seed_user(
                &db.pool,
                Uuid::parse_str(FIXTURE_AUTH_ID).expect("valid fixture UUID"),
                DB_PROFILE_NAME,
            ) {
                Ok(()) => {
                    world.db = Some(db);
                    world.skip_reason = None;
                }
                Err(error) => {
                    let _ = handle_cluster_setup_failure::<()>(error.as_str());
                    world.skip_reason = Some(error);
                }
            }
        }
        Err(error) => {
            let _ = handle_cluster_setup_failure::<()>(error.as_str());
            world.skip_reason = Some(error);
        }
    }
}

#[then("all responses match DB-backed contracts")]
fn all_responses_match_db_backed_contracts(world: &mut World) {
    if is_skipped(world) {
        return;
    }

    // Login should succeed with fixture credentials (fixture login service
    // still used even in DB mode per current architecture)
    let login = world.login.as_ref().expect("login snapshot");
    assert_eq!(login.status, 200);
    assert!(login.session_cookie.is_some());

    // Profile should return DB display name
    let profile = world.profile.as_ref().expect("profile snapshot");
    assert_profile_response(profile, DB_PROFILE_NAME);

    // Preferences should return 200 with DB-backed data
    let preferences = world.preferences.as_ref().expect("preferences snapshot");
    assert_eq!(preferences.status, 200);
    let prefs_body = preferences.body.as_ref().expect("preferences body");
    assert_eq!(
        prefs_body.get("userId").and_then(|v| v.as_str()),
        Some(FIXTURE_AUTH_ID)
    );
    // DB-backed preferences include all required fields
    assert!(prefs_body.get("interestThemeIds").is_some());
    assert!(prefs_body.get("safetyToggleIds").is_some());
    assert!(prefs_body.get("unitSystem").is_some());
    assert!(prefs_body.get("revision").is_some());

    // Catalogue explore should return 200 with DB-backed data
    let catalogue_explore = world
        .catalogue_explore
        .as_ref()
        .expect("catalogue_explore snapshot");
    assert_eq!(catalogue_explore.status, 200);
    let explore_body = catalogue_explore
        .body
        .as_ref()
        .expect("catalogue_explore body");
    assert!(explore_body.get("generatedAt").is_some());
    assert!(explore_body.get("categories").is_some());

    // Catalogue descriptors should return 200 with DB-backed data
    let catalogue_descriptors = world
        .catalogue_descriptors
        .as_ref()
        .expect("catalogue_descriptors snapshot");
    assert_eq!(catalogue_descriptors.status, 200);
    let descriptors_body = catalogue_descriptors
        .body
        .as_ref()
        .expect("catalogue_descriptors body");
    assert!(descriptors_body.get("generatedAt").is_some());
    assert!(descriptors_body.get("tags").is_some());

    // Offline bundles should return 200 with DB-backed data (empty unless
    // seeded)
    let offline_bundles = world
        .offline_bundles
        .as_ref()
        .expect("offline_bundles snapshot");
    assert_eq!(offline_bundles.status, 200);
    let bundles_body = offline_bundles
        .body
        .as_ref()
        .expect("offline_bundles body");
    assert!(bundles_body.get("bundles").is_some());

    // Enrichment provenance should return 200 with DB-backed data
    let enrichment = world
        .enrichment_provenance
        .as_ref()
        .expect("enrichment_provenance snapshot");
    assert_eq!(enrichment.status, 200);
    let enrichment_body = enrichment
        .body
        .as_ref()
        .expect("enrichment_provenance body");
    assert!(enrichment_body.get("records").is_some());
}

// ------------------------------------------------------------------------
// Scenario 3: Schema loss unhappy path
// ------------------------------------------------------------------------

#[given("the users table is missing in db-present mode")]
fn the_users_table_is_missing_in_db_present_mode(world: &mut World) {
    if is_skipped(world) {
        return;
    }

    let db = world.db.as_ref().expect("db context");
    drop_users_table(db.database_url.as_str()).expect("drop users table");
}

#[when("executing requests against user-dependent endpoints")]
fn executing_requests_against_user_dependent_endpoints(world: &mut World) {
    run_comprehensive_flow(world);
}

#[then("responses produce stable error envelopes rather than fixture data")]
fn responses_produce_stable_error_envelopes_rather_than_fixture_data(world: &mut World) {
    if is_skipped(world) {
        return;
    }

    // Login should still work (fixture login service)
    let login = world.login.as_ref().expect("login snapshot");
    assert_eq!(login.status, 200);

    // Profile should return 500 internal error (not fixture data) because
    // users table is missing
    let profile = world.profile.as_ref().expect("profile snapshot");
    assert_internal(profile);

    // Preferences should return 500 internal error (not fixture data)
    let preferences = world.preferences.as_ref().expect("preferences snapshot");
    assert_internal(preferences);
}

// ------------------------------------------------------------------------
// Scenario 4: Validation error stability
// ------------------------------------------------------------------------

#[when("executing requests with invalid input against endpoints")]
fn executing_requests_with_invalid_input_against_endpoints(world: &mut World) {
    run_validation_error_flow(world);
}

#[then("validation error envelopes are identical to db-present validation errors")]
fn validation_error_envelopes_are_identical_to_db_present_validation_errors(world: &mut World) {
    if is_skipped(world) {
        return;
    }

    // Validation errors should be 400 Bad Request regardless of startup mode
    let preferences = world.preferences.as_ref().expect("preferences snapshot");
    assert_eq!(preferences.status, 400);
    let body = preferences.body.as_ref().expect("error body");
    assert_eq!(body.get("code").and_then(|v| v.as_str()), Some("invalid_request"));
    // Should include details about missing fields
    assert!(body.get("details").is_some());
}

#[then("validation error envelopes remain stable")]
fn validation_error_envelopes_remain_stable(world: &mut World) {
    if is_skipped(world) {
        return;
    }

    // Same assertion as above - validation errors are mode-independent
    let preferences = world.preferences.as_ref().expect("preferences snapshot");
    assert_eq!(preferences.status, 400);
    let body = preferences.body.as_ref().expect("error body");
    assert_eq!(body.get("code").and_then(|v| v.as_str()), Some("invalid_request"));
    assert!(body.get("details").is_some());
}

#[scenario(
    path = "tests/features/startup_mode_composition.feature",
    name = "Fixture-fallback startup preserves fixture contracts for all port groups"
)]
fn fixture_fallback_happy_path(world: World) {
    drop(world);
}

#[scenario(
    path = "tests/features/startup_mode_composition.feature",
    name = "DB-present startup preserves DB-backed contracts for all port groups"
)]
fn db_present_happy_path(world: World) {
    drop(world);
}

#[scenario(
    path = "tests/features/startup_mode_composition.feature",
    name = "DB-present startup produces stable error envelopes when critical schemas are missing"
)]
fn schema_loss_unhappy_path(world: World) {
    drop(world);
}

#[scenario(
    path = "tests/features/startup_mode_composition.feature",
    name = "Validation error envelopes remain stable across both startup modes"
)]
fn validation_stability_edge_path(world: World) {
    drop(world);
}
