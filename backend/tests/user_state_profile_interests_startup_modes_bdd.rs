//! Behaviour coverage for 3.5.3 profile/interests startup-mode stability.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use serde_json::Value;
use uuid::Uuid;

mod support;

use support::profile_interests::{
    DB_PROFILE_NAME, FIRST_THEME_ID, FIXTURE_AUTH_ID, FIXTURE_PROFILE_NAME, INTEREST_THEME_IDS_MAX,
    SECOND_THEME_ID,
};
use support::{drop_table, handle_cluster_setup_failure};

#[path = "../src/server/config.rs"]
#[expect(
    dead_code,
    reason = "tests import ServerConfig from server_config for BDD startup-mode checks"
)]
mod server_config;
pub(crate) use server_config::ServerConfig;

#[path = "../src/server/state_builders.rs"]
mod state_builders;

#[path = "user_state_profile_interests_startup_modes_bdd/flow_support.rs"]
mod flow_support;

use flow_support::{
    World, assert_interests_response, assert_internal, assert_profile_response, is_skipped,
    run_profile_interests_flow, seed_user, setup_db_context,
};

#[fixture]
fn world() -> World {
    World {
        db: None,
        login: None,
        profile: None,
        interests: None,
        interests_payload: backend::inbound::http::users::InterestsRequest {
            interest_theme_ids: vec![FIRST_THEME_ID.to_owned()],
            expected_revision: None,
        },
        skip_reason: None,
    }
}

#[given("db-present startup mode backed by embedded postgres")]
fn db_present_startup_mode_backed_by_embedded_postgres(world: &mut World) {
    match setup_db_context() {
        Ok(db) => {
            seed_user(
                db.database_url.as_str(),
                Uuid::parse_str(FIXTURE_AUTH_ID).expect("valid fixture UUID"),
                DB_PROFILE_NAME,
            )
            .expect("seed db user");
            world.db = Some(db);
            world.skip_reason = None;
        }
        Err(error) => {
            let _ = handle_cluster_setup_failure::<()>(error.as_str());
            world.skip_reason = Some(error);
        }
    }
}

#[given("fixture-fallback startup mode without a database pool")]
fn fixture_fallback_startup_mode_without_a_database_pool(world: &mut World) {
    world.db = None;
    world.skip_reason = None;
}

#[given("the interests schema is missing in db-present mode")]
fn the_interests_schema_is_missing_in_db_present_mode(world: &mut World) {
    if is_skipped(world) {
        return;
    }

    let db = world.db.as_ref().expect("db context");
    drop_table(db.database_url.as_str(), "user_preferences").expect("drop user_preferences");
    drop_table(db.database_url.as_str(), "user_interest_themes")
        .expect("drop user_interest_themes");
}

#[when("executing a valid login, profile, and interests request")]
fn executing_a_valid_login_profile_and_interests_request(world: &mut World) {
    world.interests_payload = backend::inbound::http::users::InterestsRequest {
        interest_theme_ids: vec![FIRST_THEME_ID.to_owned()],
        expected_revision: None,
    };
    run_profile_interests_flow(world);
}

#[when("executing a valid login, profile, and interests request with multiple interestThemeIds")]
fn executing_a_valid_login_profile_and_interests_request_with_multiple_interest_theme_ids(
    world: &mut World,
) {
    world.interests_payload = backend::inbound::http::users::InterestsRequest {
        interest_theme_ids: vec![FIRST_THEME_ID.to_owned(), SECOND_THEME_ID.to_owned()],
        expected_revision: None,
    };
    run_profile_interests_flow(world);
}

#[when("executing a login, profile, and interests request with too many interestThemeIds")]
fn executing_a_login_profile_and_interests_request_with_too_many_interest_theme_ids(
    world: &mut World,
) {
    world.interests_payload = backend::inbound::http::users::InterestsRequest {
        interest_theme_ids: vec![FIRST_THEME_ID.to_owned(); INTEREST_THEME_IDS_MAX + 1],
        expected_revision: None,
    };
    run_profile_interests_flow(world);
}

fn assert_interests_status(interests: &flow_support::Snapshot) {
    match interests.status {
        200 => {
            let body = interests.body.as_ref().expect("interests body");
            assert_eq!(
                body.get("userId").and_then(Value::as_str),
                Some(FIXTURE_AUTH_ID)
            );
            let ids = body
                .get("interestThemeIds")
                .and_then(Value::as_array)
                .expect("interestThemeIds array");
            assert!(!ids.is_empty(), "interestThemeIds should stay non-empty");
        }
        500 => assert_internal(interests),
        other => panic!("unexpected /users/me/interests status after schema loss: {other}"),
    }
}

#[then("the responses preserve a stable startup error or fallback contract")]
fn the_responses_preserve_a_stable_startup_error_or_fallback_contract(world: &mut World) {
    if is_skipped(world) {
        return;
    }

    let login_snapshot = world.login.as_ref().expect("login response");
    if login_snapshot.status == 500 {
        assert_internal(login_snapshot);
        return;
    }

    assert_eq!(login_snapshot.status, 200);
    assert_eq!(
        world.profile.as_ref().expect("profile response").status,
        200
    );
    let interests = world.interests.as_ref().expect("interests response");
    assert_interests_status(interests);
}

#[then("the interests validation error envelope remains stable")]
fn the_interests_validation_error_envelope_remains_stable(world: &mut World) {
    if is_skipped(world) {
        return;
    }

    assert_eq!(world.login.as_ref().expect("login response").status, 200);
    assert_eq!(
        world.profile.as_ref().expect("profile response").status,
        200
    );

    let interests = world.interests.as_ref().expect("interests response");
    assert_eq!(interests.status, 400);
    let body = interests.body.as_ref().expect("error body");
    assert_eq!(
        body.get("message").and_then(Value::as_str),
        Some("interest theme ids must contain at most 100 items")
    );
    assert_eq!(
        body.get("code").and_then(Value::as_str),
        Some("invalid_request")
    );
    let details = body
        .get("details")
        .and_then(Value::as_object)
        .expect("details object");
    assert_eq!(
        details.get("code").and_then(Value::as_str),
        Some("too_many_interest_theme_ids")
    );
    assert_eq!(
        details.get("field").and_then(Value::as_str),
        Some("interestThemeIds")
    );
    assert_eq!(
        details.get("max").and_then(Value::as_u64),
        Some(INTEREST_THEME_IDS_MAX as u64)
    );
}

fn assert_profile_and_interests_contract(
    world: &World,
    expected_profile_name: &str,
    expected_interest_ids: &[&str],
) {
    if is_skipped(world) {
        return;
    }

    assert_eq!(world.login.as_ref().expect("login response").status, 200);
    assert_profile_response(
        world.profile.as_ref().expect("profile response"),
        expected_profile_name,
    );
    assert_interests_response(
        world.interests.as_ref().expect("interests response"),
        expected_interest_ids,
        1,
    );
}

#[then("fixture-fallback startup preserves the fixture profile and interests response contract")]
fn fixture_fallback_startup_preserves_the_fixture_profile_and_interests_response_contract(
    world: &mut World,
) {
    assert_profile_and_interests_contract(world, FIXTURE_PROFILE_NAME, &[FIRST_THEME_ID]);
}

#[then("db-present startup preserves the DB-backed profile and interests response contract")]
fn db_present_startup_preserves_the_db_backed_profile_and_interests_response_contract(
    world: &mut World,
) {
    assert_profile_and_interests_contract(
        world,
        DB_PROFILE_NAME,
        &[FIRST_THEME_ID, SECOND_THEME_ID],
    );
}

#[scenario(
    path = "tests/features/user_state_profile_interests_startup_modes.feature",
    name = "DB-present startup remains stable when interests schema is missing"
)]
fn db_present_startup_remains_stable_when_interests_schema_is_missing(world: World) {
    drop(world);
}

#[scenario(
    path = "tests/features/user_state_profile_interests_startup_modes.feature",
    name = "DB-present startup keeps interestThemeIds validation envelope stable"
)]
fn db_present_startup_keeps_interest_theme_ids_validation_envelope_stable(world: World) {
    drop(world);
}

#[scenario(
    path = "tests/features/user_state_profile_interests_startup_modes.feature",
    name = "Fixture-fallback startup keeps profile and interests response contracts stable"
)]
fn fixture_fallback_startup_keeps_profile_and_interests_response_contracts_stable(world: World) {
    drop(world);
}

#[scenario(
    path = "tests/features/user_state_profile_interests_startup_modes.feature",
    name = "DB-present startup preserves DB-backed profile and interests responses"
)]
fn db_present_startup_preserves_db_backed_profile_and_interests_responses(world: World) {
    drop(world);
}
