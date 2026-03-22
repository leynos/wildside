//! Behavioural tests for the pagination crate foundation.

#![expect(
    clippy::expect_used,
    reason = "test code uses expect for clear failure messages"
)]

use pagination::{Cursor, CursorError, PageParams, Paginated, PaginationLinks};
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FixtureKey {
    created_at: String,
    id: String,
}

#[derive(Debug, Default, ScenarioState)]
struct World {
    key: Slot<FixtureKey>,
    cursor_token: Slot<String>,
    decode_result: Slot<Result<Cursor<FixtureKey>, CursorError>>,
    page_params: Slot<PageParams>,
    request_url: Slot<Url>,
    page: Slot<Paginated<String>>,
}

#[fixture]
fn world() -> World {
    World::default()
}

#[given("a composite ordering key")]
fn a_composite_ordering_key(world: &World) {
    world.key.set(FixtureKey {
        created_at: "2026-03-22T10:30:00Z".to_owned(),
        id: "8b116c56-0a58-4c55-b7d7-06ee6bbddb8c".to_owned(),
    });
}

#[given("a malformed opaque cursor token")]
fn a_malformed_opaque_cursor_token(world: &World) {
    world.cursor_token.set("not-valid-base64".to_owned());
}

#[given("pagination parameters without a limit")]
fn pagination_parameters_without_a_limit(world: &World) {
    let params = PageParams::new(None, None).expect("default params should be valid");
    world.page_params.set(params);
}

#[given("normalized pagination parameters with cursor {cursor}")]
fn normalized_pagination_parameters_with_cursor(world: &World, cursor: String) {
    let params = PageParams::new(Some(cursor), Some(25)).expect("params should be valid");
    world.page_params.set(params);
}

#[given("a request URL with filter query parameters")]
fn a_request_url_with_filter_query_parameters(world: &World) {
    let url = Url::parse(
        "https://example.test/api/v1/users?role=admin&status=active&limit=1&cursor=stale",
    )
    .expect("request url should be valid");
    world.request_url.set(url);
}

#[when("the key is encoded into an opaque cursor and decoded again")]
fn the_key_is_encoded_into_an_opaque_cursor_and_decoded_again(world: &World) {
    let key = world.key.get().expect("key should be set");
    let token = Cursor::new(key.clone())
        .encode()
        .expect("cursor encoding should succeed");
    world.cursor_token.set(token.clone());
    world
        .decode_result
        .set(Cursor::<FixtureKey>::decode(&token));
}

#[when("the cursor is decoded")]
fn the_cursor_is_decoded(world: &World) {
    let token = world
        .cursor_token
        .get()
        .expect("cursor token should be set")
        .clone();
    world
        .decode_result
        .set(Cursor::<FixtureKey>::decode(&token));
}

#[when("the parameters are normalized")]
fn the_parameters_are_normalized(_world: &World) {}

#[when("pagination parameters request limit {limit:u64}")]
fn pagination_parameters_request_limit(world: &World, limit: u64) {
    let requested_limit = usize::try_from(limit).expect("fixture limit should fit usize");
    let params = PageParams::new(None, Some(requested_limit)).expect("params should be valid");
    world.page_params.set(params);
}

#[when("a paginated envelope is built with next and prev cursors")]
fn a_paginated_envelope_is_built_with_next_and_prev_cursors(world: &World) {
    let params = world.page_params.get().expect("page params should be set");
    let request_url = world.request_url.get().expect("request url should be set");
    let page = Paginated::new(
        vec!["Ada".to_owned(), "Linus".to_owned()],
        params.limit(),
        PaginationLinks::from_request(
            &request_url,
            &params,
            Some("next-token"),
            Some("prev-token"),
        ),
    );
    world.page.set(page);
}

#[then("the decoded cursor key matches the original key")]
fn the_decoded_cursor_key_matches_the_original_key(world: &World) {
    let key = world.key.get().expect("key should be set");
    let decode_result = world
        .decode_result
        .get()
        .expect("decode result should be set");
    let cursor = decode_result
        .as_ref()
        .expect("cursor decoding should succeed");

    assert_eq!(cursor.key(), &key);
}

#[then("cursor decoding fails")]
fn cursor_decoding_fails(world: &World) {
    let result = world
        .decode_result
        .get()
        .expect("decode result should be set");

    assert!(matches!(result, Err(CursorError::InvalidBase64 { .. })));
}

#[then("the normalized limit is {limit:u64}")]
fn the_normalized_limit_is(world: &World, limit: u64) {
    let params = world.page_params.get().expect("page params should be set");

    assert_eq!(
        u64::try_from(params.limit()).expect("limit should fit u64"),
        limit
    );
}

#[then("the self link preserves the current cursor and filter")]
fn the_self_link_preserves_the_current_cursor_and_filter(world: &World) {
    let page = world.page.get().expect("page should be set");

    assert_eq!(
        page.links.self_,
        "https://example.test/api/v1/users?role=admin&status=active&limit=25&cursor=current-token"
    );
}

#[then("the next link uses the next cursor")]
fn the_next_link_uses_the_next_cursor(world: &World) {
    let page = world.page.get().expect("page should be set");

    assert_eq!(
        page.links.next.as_deref(),
        Some(
            "https://example.test/api/v1/users?role=admin&status=active&limit=25&cursor=next-token"
        )
    );
}

#[then("the prev link uses the prev cursor")]
fn the_prev_link_uses_the_prev_cursor(world: &World) {
    let page = world.page.get().expect("page should be set");

    assert_eq!(
        page.links.prev.as_deref(),
        Some(
            "https://example.test/api/v1/users?role=admin&status=active&limit=25&cursor=prev-token"
        )
    );
}

#[scenario(path = "tests/features/pagination.feature")]
fn pagination_crate_foundation(world: World) {
    drop(world);
}
