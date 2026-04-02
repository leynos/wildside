//! Unit tests for Redis route cache using in-memory fakes.
//!
//! These tests run unconditionally and cover JSON round-trip, cache-miss,
//! and corrupt-payload semantics without requiring a live Redis server.

use rstest::rstest;

use crate::domain::ports::{RouteCache, RouteCacheError, RouteCacheKey};
use crate::outbound::cache::redis_route_cache::GenericRedisRouteCache;
use crate::outbound::cache::test_helpers::{
    FakeProvider, TestPlan, assert_put_get_round_trips_with_plan,
};

#[tokio::test]
async fn mock_get_returns_none_for_missing_key() {
    let cache = GenericRedisRouteCache::<TestPlan, _>::with_provider(FakeProvider::empty());
    let key = RouteCacheKey::new("route:missing").expect("valid key");

    let result = cache.get(&key).await.expect("get should succeed");

    assert_eq!(result, None);
}

#[tokio::test]
async fn mock_put_then_get_round_trips_the_typed_plan() {
    let cache = GenericRedisRouteCache::<TestPlan, _>::with_provider(FakeProvider::empty());
    let plan = TestPlan::new("req-1", 42);

    assert_put_get_round_trips_with_plan(&cache, &plan)
        .await
        .expect("put/get round-trip succeeds");
}

#[tokio::test]
async fn mock_corrupted_bytes_map_to_serialization_errors() {
    let cache = GenericRedisRouteCache::<TestPlan, _>::with_provider(FakeProvider::seeded(
        "route:corrupt",
        vec![0_u8, 159, 146, 150],
    ));
    let key = RouteCacheKey::new("route:corrupt").expect("valid key");

    let result = cache
        .get(&key)
        .await
        .expect_err("corrupt payload should fail");

    assert!(matches!(result, RouteCacheError::Serialization { .. }));
}

#[rstest]
#[case("not a redis url")]
#[case("http://127.0.0.1:6379")]
#[tokio::test]
async fn connect_maps_invalid_connection_strings_to_backend_errors(#[case] redis_url: &str) {
    use crate::outbound::cache::RedisRouteCache;

    let result = RedisRouteCache::<TestPlan>::connect(redis_url)
        .await
        .expect_err("invalid redis url should fail");

    assert!(matches!(result, RouteCacheError::Backend { .. }));
}
