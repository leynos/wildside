//! Behavioural coverage for keyset-paginated users listing.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

mod support;

use support::handle_cluster_setup_failure;

#[path = "users_list_pagination_bdd/flow_support.rs"]
mod flow_support;

use flow_support::{
    ORDERED_USER_IDS, World, assert_error, assert_full_traversal, assert_next_only,
    assert_prev_only, assert_status, assert_users, run_authenticated_request, run_first_page,
    run_follow_next_to_final, run_next_then_prev, run_unauthenticated_request, seed_users,
    setup_db_context, skip, store_db,
};

#[fixture]
fn world() -> World {
    World::default()
}

#[given("db-present startup mode with five ordered users")]
fn db_present_startup_mode_with_five_ordered_users(world: &mut World) {
    match setup_db_context().and_then(seed_users) {
        Ok(db) => store_db(world, db),
        Err(error) => {
            let _ = handle_cluster_setup_failure::<()>(error.as_str());
            skip(world, error);
        }
    }
}

#[when("the client requests the first users page with limit 2")]
fn the_client_requests_the_first_users_page_with_limit_2(world: &mut World) {
    run_first_page(world);
}

#[when("the client follows users next links with limit 2 until the final page")]
fn the_client_follows_users_next_links_with_limit_2_until_the_final_page(world: &mut World) {
    run_follow_next_to_final(world);
}

#[when("the client follows next then prev users links with limit 2")]
fn the_client_follows_next_then_prev_users_links_with_limit_2(world: &mut World) {
    run_next_then_prev(world);
}

#[when("the client requests the users list with limit 200")]
fn the_client_requests_the_users_list_with_limit_200(world: &mut World) {
    run_authenticated_request(world, "/api/v1/users?limit=200");
}

#[when("the client requests the users list with an invalid cursor")]
fn the_client_requests_the_users_list_with_an_invalid_cursor(world: &mut World) {
    run_authenticated_request(world, "/api/v1/users?cursor=not-a-cursor");
}

#[when("the client requests the users list without a session")]
fn the_client_requests_the_users_list_without_a_session(world: &mut World) {
    run_unauthenticated_request(world);
}

#[then("the users response is ok")]
fn the_users_response_is_ok(world: &mut World) {
    assert_status(world, 200);
}

#[then("the users page contains users 1 through 2")]
fn the_users_page_contains_users_1_through_2(world: &mut World) {
    assert_users(world, &ORDERED_USER_IDS[0..2]);
}

#[then("the users page contains users 3 through 4")]
fn the_users_page_contains_users_3_through_4(world: &mut World) {
    assert_users(world, &ORDERED_USER_IDS[2..4]);
}

#[then("the users page contains user 5 only")]
fn the_users_page_contains_user_5_only(world: &mut World) {
    assert_users(world, &ORDERED_USER_IDS[4..5]);
}

#[then("the users page includes a next link and omits the prev link")]
fn the_users_page_includes_a_next_link_and_omits_the_prev_link(world: &mut World) {
    assert_next_only(world);
}

#[then("the users page includes a prev link and omits the next link")]
fn the_users_page_includes_a_prev_link_and_omits_the_next_link(world: &mut World) {
    assert_prev_only(world);
}

#[then("forward traversal returned every seeded user once")]
fn forward_traversal_returned_every_seeded_user_once(world: &mut World) {
    assert_full_traversal(world);
}

#[then("the users response is bad request with invalid_limit details")]
fn the_users_response_is_bad_request_with_invalid_limit_details(world: &mut World) {
    assert_error(world, 400, "invalid_limit");
}

#[then("the users response is bad request with invalid_cursor details")]
fn the_users_response_is_bad_request_with_invalid_cursor_details(world: &mut World) {
    assert_error(world, 400, "invalid_cursor");
}

#[then("the users response is unauthorised")]
fn the_users_response_is_unauthorised(world: &mut World) {
    assert_status(world, 401);
}

#[scenario(
    path = "tests/features/users_list_pagination.feature",
    name = "First users page exposes the next link only"
)]
fn first_users_page_exposes_the_next_link_only(world: World) {
    drop(world);
}

#[scenario(
    path = "tests/features/users_list_pagination.feature",
    name = "Following next reaches the final users page"
)]
fn following_next_reaches_the_final_users_page(world: World) {
    drop(world);
}

#[scenario(
    path = "tests/features/users_list_pagination.feature",
    name = "Following prev from the final users page returns the prior page"
)]
fn following_prev_from_the_final_users_page_returns_the_prior_page(world: World) {
    drop(world);
}

#[scenario(
    path = "tests/features/users_list_pagination.feature",
    name = "Oversized users page limit is rejected"
)]
fn oversized_users_page_limit_is_rejected(world: World) {
    drop(world);
}

#[scenario(
    path = "tests/features/users_list_pagination.feature",
    name = "Invalid users cursor is rejected"
)]
fn invalid_users_cursor_is_rejected(world: World) {
    drop(world);
}

#[scenario(
    path = "tests/features/users_list_pagination.feature",
    name = "Users list requires a session"
)]
fn users_list_requires_a_session(world: World) {
    drop(world);
}
