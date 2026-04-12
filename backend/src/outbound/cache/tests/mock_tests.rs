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

mod ttl_integration_tests {
    //! Integration tests for TTL (time-to-live) behaviour with jittered expiry.

    use crate::domain::ports::{RouteCache, RouteCacheKey};
    use crate::outbound::cache::redis_route_cache::GenericRedisRouteCache;
    use crate::outbound::cache::test_helpers::{FakeProvider, TestPlan};

    #[tokio::test]
    async fn put_stores_entry_with_jittered_ttl_within_expected_range() {
        let provider = FakeProvider::empty();
        let cache = GenericRedisRouteCache::<TestPlan, _>::with_provider_and_ttl(
            provider.clone(),
            86_400,
            0.10,
        );
        let key = RouteCacheKey::new("route:ttl-test").expect("valid key");
        let plan = TestPlan::new("req-1", 100);

        cache.put(&key, &plan).await.expect("put should succeed");

        let ttl = provider.ttl_for("route:ttl-test");
        assert!(ttl.is_some(), "TTL should be recorded");
        let ttl = ttl.expect("TTL exists");
        assert!(ttl >= 77_760, "TTL {ttl} below lower bound 77760");
        assert!(ttl <= 95_040, "TTL {ttl} above upper bound 95040");
    }

    #[tokio::test]
    async fn put_with_zero_jitter_uses_exact_base_ttl() {
        let provider = FakeProvider::empty();
        let base = 1000;
        let cache = GenericRedisRouteCache::<TestPlan, _>::with_provider_and_ttl(
            provider.clone(),
            base,
            0.0,
        );
        let key = RouteCacheKey::new("route:no-jitter").expect("valid key");
        let plan = TestPlan::default();

        cache.put(&key, &plan).await.expect("put should succeed");

        let ttl = provider.ttl_for("route:no-jitter");
        assert_eq!(ttl, Some(base), "Zero jitter should use exact base TTL");
    }

    #[tokio::test]
    async fn put_followed_by_get_still_round_trips_correctly() {
        let provider = FakeProvider::empty();
        let cache =
            GenericRedisRouteCache::<TestPlan, _>::with_provider_and_ttl(provider, 3600, 0.10);
        let key = RouteCacheKey::new("route:round-trip-with-ttl").expect("valid key");
        let plan = TestPlan::new("req-2", 200);

        cache.put(&key, &plan).await.expect("put should succeed");
        let loaded = cache.get(&key).await.expect("get should succeed");

        assert_eq!(loaded, Some(plan));
    }
}

mod jitter_tests {
    //! Unit tests for the jittered TTL calculation helper function.

    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use rstest::rstest;

    use crate::outbound::cache::jittered_ttl;

    #[rstest]
    #[case(42)]
    #[case(100)]
    #[case(999)]
    fn jittered_ttl_stays_within_bounds_for_24h_base_and_10_percent_jitter(#[case] seed: u64) {
        let mut rng = StdRng::seed_from_u64(seed);
        let base = 86_400;
        let jitter = 0.10;

        let ttl = jittered_ttl(base, jitter, &mut rng);

        assert!(ttl >= 77_760, "TTL {ttl} below lower bound 77760");
        assert!(ttl <= 95_040, "TTL {ttl} above upper bound 95040");
    }

    #[test]
    fn jittered_ttl_returns_at_least_one_for_zero_base() {
        let mut rng = StdRng::seed_from_u64(0);

        let ttl = jittered_ttl(0, 0.10, &mut rng);

        assert_eq!(ttl, 1, "Zero base must clamp to 1");
    }

    #[test]
    fn jittered_ttl_returns_exact_base_for_zero_jitter() {
        let mut rng = StdRng::seed_from_u64(123);
        let base = 1000;

        let ttl = jittered_ttl(base, 0.0, &mut rng);

        assert_eq!(ttl, base, "Zero jitter must return exact base");
    }

    #[test]
    fn jittered_ttl_handles_large_jitter_without_overflow() {
        let mut rng = StdRng::seed_from_u64(456);
        let base = 86_400;
        let jitter = 1.0;

        let ttl = jittered_ttl(base, jitter, &mut rng);

        assert!(ttl >= 1, "Large jitter must still clamp to at least 1");
        assert!(ttl <= 2 * base, "Large jitter must not exceed 2 * base");
    }

    #[test]
    fn jittered_ttl_produces_varying_values_across_multiple_calls() {
        let mut rng = StdRng::seed_from_u64(789);
        let base = 86_400;
        let jitter = 0.10;

        let mut values = Vec::new();
        for _ in 0..10 {
            values.push(jittered_ttl(base, jitter, &mut rng));
        }

        let first = values[0];
        let all_same = values.iter().all(|&v| v == first);
        assert!(!all_same, "Jittered TTL must vary across calls");
    }
}
