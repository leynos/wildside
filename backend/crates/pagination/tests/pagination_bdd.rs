//! Behavioural tests for the pagination crate foundation.

mod common;

use common::{Cursor, CursorError, Direction, FixtureKey, World};
use pagination::{PageParams, Paginated, PaginationLinks};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use serde_json::Value;
use url::Url;

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
    world.cursor_token.set("not!valid".to_owned());
}

#[given("pagination parameters without a limit")]
#[expect(
    clippy::expect_used,
    reason = "BDD steps use expect for clear failures"
)]
fn pagination_parameters_without_a_limit(world: &World) {
    let params = PageParams::new(None, None).expect("default params should be valid");
    world.page_params.set(params);
}

#[given("normalized pagination parameters with cursor {cursor}")]
#[expect(
    clippy::expect_used,
    reason = "BDD steps use expect for clear failures"
)]
fn normalized_pagination_parameters_with_cursor(world: &World, cursor: String) {
    let params = PageParams::new(Some(cursor), Some(25)).expect("params should be valid");
    world.page_params.set(params);
}

#[given("a request URL with filter query parameters")]
#[expect(
    clippy::expect_used,
    reason = "BDD steps use expect for clear failures"
)]
fn a_request_url_with_filter_query_parameters(world: &World) {
    let url = Url::parse(
        "https://example.test/api/v1/users?role=admin&status=active&limit=1&cursor=stale",
    )
    .expect("request url should be valid");
    world.request_url.set(url);
}

#[when("the key is encoded into an opaque cursor and decoded again")]
#[expect(
    clippy::expect_used,
    reason = "BDD steps use expect for clear failures"
)]
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
#[expect(
    clippy::expect_used,
    reason = "BDD steps use expect for clear failures"
)]
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

#[when("pagination parameters request limit {limit:u64}")]
#[expect(
    clippy::expect_used,
    reason = "BDD steps use expect for clear failures"
)]
fn pagination_parameters_request_limit(world: &World, limit: u64) {
    let requested_limit = usize::try_from(limit).expect("fixture limit should fit usize");
    let params = PageParams::new(None, Some(requested_limit)).expect("params should be valid");
    world.page_params.set(params);
}

/// Helper to build a paginated envelope with the given cursors.
#[expect(
    clippy::expect_used,
    reason = "BDD helpers use expect for clear failures"
)]
fn build_paginated_envelope(
    world: &World,
    next_cursor: Option<&str>,
    prev_cursor: Option<&str>,
) -> Paginated<String> {
    let params = world.page_params.get().expect("page params should be set");
    let request_url = world.request_url.get().expect("request url should be set");
    Paginated::new(
        vec!["Ada".to_owned(), "Linus".to_owned()],
        params.limit(),
        PaginationLinks::from_request(&request_url, &params, next_cursor, prev_cursor),
    )
}

#[when("a paginated envelope is built with next and prev cursors")]
fn a_paginated_envelope_is_built_with_next_and_prev_cursors(world: &World) {
    let page = build_paginated_envelope(world, Some("next-token"), Some("prev-token"));
    world.page.set(page);
}

#[when("a paginated envelope is built with only a next cursor")]
fn a_paginated_envelope_is_built_with_only_a_next_cursor(world: &World) {
    let page = build_paginated_envelope(world, Some("next-token"), None);
    world.page.set(page);
}

#[when("a paginated envelope is built with only a prev cursor")]
fn a_paginated_envelope_is_built_with_only_a_prev_cursor(world: &World) {
    let page = build_paginated_envelope(world, None, Some("prev-token"));
    world.page.set(page);
}

#[when("a paginated envelope is built without pagination cursors")]
fn a_paginated_envelope_is_built_without_pagination_cursors(world: &World) {
    let page = build_paginated_envelope(world, None, None);
    world.page.set(page);
}

#[then("the decoded cursor key matches the original key")]
#[expect(
    clippy::expect_used,
    reason = "BDD steps use expect for clear failures"
)]
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
#[expect(
    clippy::expect_used,
    reason = "BDD steps use expect for clear failures"
)]
fn cursor_decoding_fails(world: &World) {
    let result = world
        .decode_result
        .get()
        .expect("decode result should be set");

    assert!(matches!(result, Err(CursorError::InvalidBase64 { .. })));
}

#[then("the normalized limit is {limit:u64}")]
#[expect(
    clippy::expect_used,
    reason = "BDD steps use expect for clear failures"
)]
fn the_normalized_limit_is(world: &World, limit: u64) {
    let params = world.page_params.get().expect("page params should be set");

    assert_eq!(
        u64::try_from(params.limit()).expect("limit should fit u64"),
        limit
    );
}

#[then("the self link preserves the current cursor and filter")]
#[expect(
    clippy::expect_used,
    reason = "BDD steps use expect for clear failures"
)]
fn the_self_link_preserves_the_current_cursor_and_filter(world: &World) {
    let page = world.page.get().expect("page should be set");

    assert_eq!(
        page.links.self_,
        "https://example.test/api/v1/users?role=admin&status=active&limit=25&cursor=current-token"
    );
}

#[then("the next link uses the next cursor")]
#[expect(
    clippy::expect_used,
    reason = "BDD steps use expect for clear failures"
)]
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
#[expect(
    clippy::expect_used,
    reason = "BDD steps use expect for clear failures"
)]
fn the_prev_link_uses_the_prev_cursor(world: &World) {
    let page = world.page.get().expect("page should be set");

    assert_eq!(
        page.links.prev.as_deref(),
        Some(
            "https://example.test/api/v1/users?role=admin&status=active&limit=25&cursor=prev-token"
        )
    );
}

#[then("the next link is omitted from the envelope")]
fn the_next_link_is_omitted_from_the_envelope(world: &World) {
    let links = serialized_links(world);

    assert!(links.get("next").is_none());
}

#[then("the prev link is omitted from the envelope")]
fn the_prev_link_is_omitted_from_the_envelope(world: &World) {
    let links = serialized_links(world);

    assert!(links.get("prev").is_none());
}

#[expect(
    clippy::expect_used,
    reason = "BDD helpers use expect for clear failures"
)]
fn serialized_links(world: &World) -> serde_json::Map<String, Value> {
    let page = world.page.get().expect("page should be set");
    let payload = serde_json::to_value(page).expect("paginated envelope should serialize");
    let links = payload
        .get("links")
        .and_then(Value::as_object)
        .expect("serialized payload should include links object");

    assert!(links.get("self").is_some());

    links.clone()
}

// Direction-aware cursor step definitions

#[given("pagination direction Next")]
fn pagination_direction_next(world: &World) {
    world.direction.set(Direction::Next);
}

#[given("pagination direction Prev")]
fn pagination_direction_prev(world: &World) {
    world.direction.set(Direction::Prev);
}

#[when("the key and direction are encoded into a cursor and decoded")]
#[expect(
    clippy::expect_used,
    reason = "BDD steps use expect for clear failures"
)]
fn the_key_and_direction_are_encoded_into_a_cursor_and_decoded(world: &World) {
    let key = world.key.get().expect("key should be set");
    let dir = world.direction.get().expect("direction should be set");
    let token = Cursor::with_direction(key.clone(), dir)
        .encode()
        .expect("cursor encoding should succeed");
    world.cursor_token.set(token.clone());
    world
        .decode_result
        .set(Cursor::<FixtureKey>::decode(&token));
}

#[then("the decoded cursor has direction Next")]
#[expect(
    clippy::expect_used,
    reason = "BDD steps use expect for clear failures"
)]
fn the_decoded_cursor_has_direction_next(world: &World) {
    let decode_result = world
        .decode_result
        .get()
        .expect("decode result should be set");
    let cursor = decode_result
        .as_ref()
        .expect("cursor decoding should succeed");

    assert_eq!(cursor.direction(), Direction::Next);
}

#[then("the decoded cursor has direction Prev")]
#[expect(
    clippy::expect_used,
    reason = "BDD steps use expect for clear failures"
)]
fn the_decoded_cursor_has_direction_prev(world: &World) {
    let decode_result = world
        .decode_result
        .get()
        .expect("decode result should be set");
    let cursor = decode_result
        .as_ref()
        .expect("cursor decoding should succeed");

    assert_eq!(cursor.direction(), Direction::Prev);
}

#[scenario(path = "tests/features/pagination.feature")]
fn pagination_crate_foundation(world: World) {
    drop(world);
}

#[scenario(path = "tests/features/direction_aware_cursors.feature")]
fn direction_aware_cursor_pagination(world: World) {
    drop(world);
}
