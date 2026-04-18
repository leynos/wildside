//! Behavioural coverage for revision-safe interests updates against real DB wiring.

pub(crate) use backend::test_support::server::{ServerConfig, build_http_state};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use uuid::Uuid;

mod support;

use support::handle_cluster_setup_failure;

#[path = "user_interests_revision_conflicts_bdd/flow_support.rs"]
mod flow_support;

use flow_support::{
    ExpectedPreferences, FIRST_THEME_ID, SAFETY_TOGGLE_ID, SECOND_THEME_ID, SeedPreferences,
    THIRD_THEME_ID, World, assert_conflict_snapshot, assert_interests_snapshot,
    assert_preferences_snapshot, is_skipped, run_first_write, run_matching_revision_write,
    run_missing_revision_conflict, run_preserve_non_interest_flow, run_stale_revision_conflict,
    seed_preferences, seed_user, setup_db_context,
};
use support::profile_interests::FIXTURE_AUTH_ID;

fn with_world<F: FnOnce(&mut World)>(world: &mut World, f: F) {
    if !is_skipped(world) {
        f(world);
    }
}

fn then_first_update_is_interests(world: &mut World, ids: &[&str], revision: u32) {
    with_world(world, |w| {
        assert_interests_snapshot(
            w.first_update.as_ref().expect("first update response"),
            ids,
            revision,
        );
    });
}

fn then_second_update_is_interests(world: &mut World, ids: &[&str], revision: u32) {
    with_world(world, |w| {
        assert_interests_snapshot(
            w.second_update.as_ref().expect("second update response"),
            ids,
            revision,
        );
    });
}

fn then_first_update_is_conflict(world: &mut World, expected: Option<u32>, actual: u32) {
    with_world(world, |w| {
        assert_conflict_snapshot(
            w.first_update.as_ref().expect("conflict response"),
            expected,
            actual,
        );
    });
}

#[fixture]
fn world() -> World {
    World::default()
}

fn seed_prefs(world: &mut World, prefs: SeedPreferences<'_>) {
    let db = world.db.as_ref().expect("db context");
    let user_id = Uuid::parse_str(FIXTURE_AUTH_ID).expect("valid fixture UUID");
    seed_preferences(db.database_url.as_str(), user_id, prefs).expect("seed user preferences");
}

#[given("db-present startup mode backed by embedded postgres")]
fn db_present_startup_mode_backed_by_embedded_postgres(world: &mut World) {
    match setup_db_context() {
        Ok(db) => {
            seed_user(
                db.database_url.as_str(),
                Uuid::parse_str(FIXTURE_AUTH_ID).expect("valid fixture UUID"),
                "Revision Ada",
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

#[given("existing preferences revision 1 with preserved safety and unit settings")]
fn existing_preferences_revision_1_with_preserved_safety_and_unit_settings(world: &mut World) {
    with_world(world, |world| {
        seed_prefs(
            world,
            SeedPreferences {
                interest_ids: &[FIRST_THEME_ID],
                safety_ids: &[SAFETY_TOGGLE_ID],
                unit_system: "imperial",
                revision: 1,
            },
        );
    });
}

#[given("existing preferences revision 2")]
fn existing_preferences_revision_2(world: &mut World) {
    with_world(world, |world| {
        seed_prefs(
            world,
            SeedPreferences {
                interest_ids: &[FIRST_THEME_ID],
                safety_ids: &[SAFETY_TOGGLE_ID],
                unit_system: "metric",
                revision: 2,
            },
        );
    });
}

#[when("the client writes interests for the first time")]
fn the_client_writes_interests_for_the_first_time(world: &mut World) {
    run_first_write(world);
}

#[when("the client writes interests twice using the returned revision")]
fn the_client_writes_interests_twice_using_the_returned_revision(world: &mut World) {
    run_matching_revision_write(world);
}

#[when("the client writes interests with stale expected revision 1")]
fn the_client_writes_interests_with_stale_expected_revision_1(world: &mut World) {
    run_stale_revision_conflict(world);
}

#[when("the client writes interests without expected revision after preferences exist")]
fn the_client_writes_interests_without_expected_revision_after_preferences_exist(
    world: &mut World,
) {
    run_missing_revision_conflict(world);
}

#[when("the client updates interests and then fetches preferences")]
fn the_client_updates_interests_and_then_fetches_preferences(world: &mut World) {
    run_preserve_non_interest_flow(world);
}

#[then("the first interests response includes revision 1")]
fn the_first_interests_response_includes_revision_1(world: &mut World) {
    then_first_update_is_interests(world, &[FIRST_THEME_ID], 1);
}

#[then("the second interests response includes revision 2")]
fn the_second_interests_response_includes_revision_2(world: &mut World) {
    then_second_update_is_interests(world, &[SECOND_THEME_ID], 2);
}

#[then("the response is a conflict with expected revision 1 and actual revision 2")]
fn the_response_is_a_conflict_with_expected_revision_1_and_actual_revision_2(world: &mut World) {
    then_first_update_is_conflict(world, Some(1), 2);
}

#[then("the response is a conflict with missing expected revision and actual revision 1")]
fn the_response_is_a_conflict_with_missing_expected_revision_and_actual_revision_1(
    world: &mut World,
) {
    then_first_update_is_conflict(world, None, 1);
}

#[then("the fetched preferences preserve safety and unit settings while advancing revision 2")]
fn the_fetched_preferences_preserve_safety_and_unit_settings_while_advancing_revision_2(
    world: &mut World,
) {
    with_world(world, |world| {
        assert_interests_snapshot(
            world.first_update.as_ref().expect("update response"),
            &[THIRD_THEME_ID],
            2,
        );
        assert_preferences_snapshot(
            world.preferences.as_ref().expect("preferences response"),
            ExpectedPreferences {
                interest_ids: &[THIRD_THEME_ID],
                safety_ids: &[SAFETY_TOGGLE_ID],
                unit_system: "imperial",
                revision: 2,
            },
        );
    });
}

#[scenario(
    path = "tests/features/user_interests_revision_conflicts.feature",
    name = "First interests write creates revision 1"
)]
fn first_interests_write_creates_revision_1(world: World) {
    drop(world);
}

#[scenario(
    path = "tests/features/user_interests_revision_conflicts.feature",
    name = "Matching expected revision advances interests revision"
)]
fn matching_expected_revision_advances_interests_revision(world: World) {
    drop(world);
}

#[scenario(
    path = "tests/features/user_interests_revision_conflicts.feature",
    name = "Stale expected revision returns a conflict"
)]
fn stale_expected_revision_returns_a_conflict(world: World) {
    drop(world);
}

#[scenario(
    path = "tests/features/user_interests_revision_conflicts.feature",
    name = "Missing expected revision after preferences exist returns a conflict"
)]
fn missing_expected_revision_after_preferences_exist_returns_a_conflict(world: World) {
    drop(world);
}

#[scenario(
    path = "tests/features/user_interests_revision_conflicts.feature",
    name = "Interests updates preserve non-interest preferences fields"
)]
fn interests_updates_preserve_non_interest_preferences_fields(world: World) {
    drop(world);
}
