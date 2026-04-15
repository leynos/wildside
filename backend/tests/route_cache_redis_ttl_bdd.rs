//! Unit-level TTL jitter tests using `FakeProvider` (no live Redis required).
//!
//! These tests verify that jittered TTL values computed by the adapter are
//! captured correctly at write time. `FakeProvider` records the intended TTL
//! for each `put` call, so assertions are drift-immune — they do not depend on
//! Redis countdown timers or wall-clock elapsed time.
//!
//! For live-Redis TTL verification see `route_cache_redis_bdd.rs`.

use std::sync::Arc;

use backend::{
    domain::ports::{RouteCache, RouteCacheKey},
    outbound::cache::{
        GenericRedisRouteCache,
        test_helpers::{FakeProvider, TestPlan},
    },
};
use rand::SeedableRng;
use rand::rngs::SmallRng;
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};

type InnerCache = GenericRedisRouteCache<TestPlan, FakeProvider>;

#[derive(Clone)]
struct CacheHandle(Arc<InnerCache>);

#[derive(Default, ScenarioState)]
struct TtlJitterWorld {
    provider: Slot<FakeProvider>,
    cache: Slot<CacheHandle>,
    test_keys: Slot<Vec<String>>,
}

impl TtlJitterWorld {
    fn cache(&self) -> CacheHandle {
        self.cache.get().expect("cache should be initialised")
    }

    fn provider(&self) -> FakeProvider {
        self.provider
            .get()
            .expect("provider should be initialised")
            .clone()
    }

    fn bootstrap_cache(&self) {
        let provider = FakeProvider::empty();
        let rng = Box::new(SmallRng::seed_from_u64(42));
        let cache = GenericRedisRouteCache::<TestPlan, _>::with_provider_and_ttl(
            provider.clone(),
            86_400, // 24 hours
            0.10,   // ±10% jitter
            rng,
        );

        self.provider.set(provider);
        self.cache.set(CacheHandle(Arc::new(cache)));
    }

    fn store_plan(&self, key: &str, plan: TestPlan) {
        let cache_key = RouteCacheKey::new(key).expect("valid key");
        let cache = self.cache();
        tokio::runtime::Runtime::new()
            .expect("create runtime")
            .block_on(async { cache.0.put(&cache_key, &plan).await })
            .expect("put should succeed");
    }

    fn get_recorded_ttls(&self) -> Vec<u64> {
        let provider = self.provider();
        let keys = self.test_keys.get().expect("test keys should be set");

        keys.iter()
            .filter_map(|key| provider.ttl_for(key).expect("ttl_for should succeed"))
            .collect()
    }
}

#[fixture]
fn world() -> TtlJitterWorld {
    TtlJitterWorld::default()
}

#[given("a running Redis-backed route cache")]
fn a_running_redis_backed_route_cache(world: &TtlJitterWorld) {
    world.bootstrap_cache();
}

#[when("five plans are stored under distinct cache keys")]
fn five_plans_are_stored_under_distinct_cache_keys(world: &TtlJitterWorld) {
    let keys: Vec<String> = (1..=5).map(|i| format!("route:jitter-{i}")).collect();

    for (i, key) in keys.iter().enumerate() {
        let plan_num = (i + 1) as u64;
        world.store_plan(key, TestPlan::new(&format!("req-{plan_num}"), plan_num));
    }

    world.test_keys.set(keys);
}

#[then("not all recorded TTLs are identical")]
fn not_all_recorded_ttls_are_identical(world: &TtlJitterWorld) {
    let ttl_values = world.get_recorded_ttls();

    assert_eq!(
        ttl_values.len(),
        5,
        "Should have recorded TTLs for all 5 keys"
    );

    let first_ttl = ttl_values[0];
    let all_same = ttl_values.iter().all(|&ttl| ttl == first_ttl);
    assert!(
        !all_same,
        "At least two TTLs should differ (jitter should vary them), got all {first_ttl}"
    );
}

#[then("all recorded TTLs fall within the configured jitter window")]
fn all_recorded_ttls_fall_within_the_configured_jitter_window(world: &TtlJitterWorld) {
    use backend::outbound::cache::{DEFAULT_BASE_TTL_SECS, DEFAULT_JITTER_FRACTION};
    let jitter = (DEFAULT_BASE_TTL_SECS as f64 * DEFAULT_JITTER_FRACTION) as u64;
    let lower = DEFAULT_BASE_TTL_SECS - jitter;
    let upper = DEFAULT_BASE_TTL_SECS + jitter;
    let ttl_values = world.get_recorded_ttls();
    for ttl in &ttl_values {
        assert!(
            *ttl >= lower && *ttl <= upper,
            "TTL {ttl}s is outside expected window [{lower}s, {upper}s]"
        );
    }
}

#[scenario(
    path = "tests/features/route_cache_redis.feature",
    name = "Jittered writes produce varying TTLs"
)]
fn jittered_writes_produce_varying_ttls(world: TtlJitterWorld) {
    drop(world);
}
