//! Behavioural tests for the Redis-backed `RouteCache` adapter.
//!
//! These tests require a `redis-server` binary on `PATH`. When the binary is
//! absent or `SKIP_REDIS_TESTS=1` is set, scenarios are skipped at runtime
//! rather than failing.

use std::sync::Arc;

use backend::{
    domain::ports::{RouteCache, RouteCacheError, RouteCacheKey},
    outbound::cache::RedisRouteCache,
};
use bb8_redis::{RedisConnectionManager, bb8::Pool};
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};
use serde::{Deserialize, Serialize};
use tokio::runtime::Runtime;

#[path = "support/redis_skip.rs"]
mod redis_skip;
#[path = "support/redis.rs"]
mod redis_support;

use backend::test_support::redis::unused_redis_url;
use redis_skip::should_skip_redis_tests;
use redis_support::RedisTestServer;

#[derive(Clone)]
struct RuntimeHandle(Arc<Runtime>);

#[derive(Clone)]
struct RedisServerHandle(Arc<RedisTestServer>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct TestPlan {
    request_id: String,
    checksum: u64,
}

impl TestPlan {
    fn new(request_id: &str, checksum: u64) -> Self {
        Self {
            request_id: request_id.to_owned(),
            checksum,
        }
    }
}

#[derive(Default, ScenarioState)]
struct RouteCacheWorld {
    runtime: Slot<RuntimeHandle>,
    server: Slot<RedisServerHandle>,
    cache: Slot<RedisRouteCache<TestPlan>>,
    first_loaded_plan: Slot<Result<Option<TestPlan>, RouteCacheError>>,
    second_loaded_plan: Slot<Result<Option<TestPlan>, RouteCacheError>>,
    latest_put_result: Slot<Result<(), RouteCacheError>>,
    latest_error: Slot<RouteCacheError>,
    skip_reason: Slot<String>,
}

impl RouteCacheWorld {
    fn runtime(&self) -> RuntimeHandle {
        self.runtime.get().expect("runtime should be initialised")
    }

    fn cache(&self) -> RedisRouteCache<TestPlan> {
        self.cache.get().expect("cache should be initialised")
    }

    fn is_skipped(&self) -> bool {
        if let Some(reason) = self.skip_reason.get() {
            eprintln!("SKIP-REDIS-TESTS: scenario skipped ({reason})");
            true
        } else {
            false
        }
    }

    fn check_skip(&self) {
        if should_skip_redis_tests() {
            self.skip_reason
                .set("redis-server unavailable or SKIP_REDIS_TESTS set".to_owned());
        }
    }

    fn setup_running_cache(&self) {
        let runtime = Runtime::new().expect("create runtime");
        let server = runtime
            .block_on(async { RedisTestServer::start().await })
            .expect("start redis-server");
        let pool = runtime
            .block_on(async { server.pool().await })
            .expect("create redis pool");
        let cache = RedisRouteCache::new(pool);

        self.runtime.set(RuntimeHandle(Arc::new(runtime)));
        self.server.set(RedisServerHandle(Arc::new(server)));
        self.cache.set(cache);
    }

    fn setup_unreachable_cache(&self) {
        let runtime = Runtime::new().expect("create runtime");
        let redis_url = runtime
            .block_on(async { unused_redis_url().await })
            .expect("unused redis url");
        let manager =
            RedisConnectionManager::new(redis_url.as_str()).expect("create redis manager");
        let pool = {
            let _guard = runtime.enter();
            Pool::builder().max_size(1).build_unchecked(manager)
        };

        self.runtime.set(RuntimeHandle(Arc::new(runtime)));
        self.cache.set(RedisRouteCache::new(pool));
    }

    fn store_plan(&self, key: &str, plan: TestPlan) {
        let key = RouteCacheKey::new(key).expect("valid key");
        let runtime = self.runtime();
        let cache = self.cache();
        let result = runtime
            .0
            .block_on(async move { cache.put(&key, &plan).await });
        self.latest_put_result.set(result);
    }

    fn load_plan_into(&self, key: &str, slot: &Slot<Result<Option<TestPlan>, RouteCacheError>>) {
        let key = RouteCacheKey::new(key).expect("valid key");
        let runtime = self.runtime();
        let cache = self.cache();
        let result = runtime.0.block_on(async move { cache.get(&key).await });
        slot.set(result);
    }

    fn seed_corrupt_bytes(&self, key: &str, bytes: Vec<u8>) {
        let server = self
            .server
            .get()
            .expect("redis server should be initialised");
        let runtime = self.runtime();
        runtime
            .0
            .block_on(async { server.0.seed_raw_bytes(key, bytes).await })
            .expect("seed corrupt bytes");
    }

    fn remember_error_from_first_get(&self) {
        let error = self
            .first_loaded_plan
            .get()
            .expect("first get result should be present")
            .as_ref()
            .expect_err("expected get to fail")
            .clone();
        self.latest_error.set(error);
    }
}

#[fixture]
fn world() -> RouteCacheWorld {
    RouteCacheWorld::default()
}

#[given("a running Redis-backed route cache")]
fn a_running_redis_backed_route_cache(world: &RouteCacheWorld) {
    world.check_skip();
    if world.is_skipped() {
        return;
    }
    world.setup_running_cache();
}

#[given("a Redis-backed route cache with malformed cached bytes")]
fn a_redis_backed_route_cache_with_malformed_cached_bytes(world: &RouteCacheWorld) {
    world.check_skip();
    if world.is_skipped() {
        return;
    }
    world.setup_running_cache();
    world.seed_corrupt_bytes("route:corrupt", vec![0_u8, 159, 146, 150]);
}

#[given("an unavailable Redis-backed route cache")]
fn an_unavailable_redis_backed_route_cache(world: &RouteCacheWorld) {
    world.setup_unreachable_cache();
}

#[when("a plan is stored under cache key \"route:happy\"")]
fn a_plan_is_stored_under_cache_key_route_happy(world: &RouteCacheWorld) {
    if world.is_skipped() {
        return;
    }
    world.store_plan("route:happy", TestPlan::new("req-happy", 42));
}

#[when("the cache is read for key \"route:happy\"")]
fn the_cache_is_read_for_key_route_happy(world: &RouteCacheWorld) {
    if world.is_skipped() {
        return;
    }
    world.load_plan_into("route:happy", &world.first_loaded_plan);
}

#[when("the cache is read for key \"route:missing\"")]
fn the_cache_is_read_for_key_route_missing(world: &RouteCacheWorld) {
    if world.is_skipped() {
        return;
    }
    world.load_plan_into("route:missing", &world.first_loaded_plan);
}

#[when("the cache is read for key \"route:corrupt\"")]
fn the_cache_is_read_for_key_route_corrupt(world: &RouteCacheWorld) {
    if world.is_skipped() {
        return;
    }
    world.load_plan_into("route:corrupt", &world.first_loaded_plan);
    world.remember_error_from_first_get();
}

#[when("the unavailable cache is read for key \"route:down\"")]
fn the_unavailable_cache_is_read_for_key_route_down(world: &RouteCacheWorld) {
    if world.is_skipped() {
        return;
    }
    world.load_plan_into("route:down", &world.first_loaded_plan);
    world.remember_error_from_first_get();
}

#[when("distinct plans are stored under cache keys \"route:first\" and \"route:second\"")]
fn distinct_plans_are_stored_under_cache_keys(world: &RouteCacheWorld) {
    if world.is_skipped() {
        return;
    }
    world.store_plan("route:first", TestPlan::new("req-first", 1));
    let first_put = world
        .latest_put_result
        .get()
        .expect("first put result should be recorded")
        .clone();
    first_put.expect("first put should succeed");

    world.store_plan("route:second", TestPlan::new("req-second", 2));
}

#[when("both cache keys are read back")]
fn both_cache_keys_are_read_back(world: &RouteCacheWorld) {
    if world.is_skipped() {
        return;
    }
    world.load_plan_into("route:first", &world.first_loaded_plan);
    world.load_plan_into("route:second", &world.second_loaded_plan);
}

#[then("the same plan is returned from the cache")]
fn the_same_plan_is_returned_from_the_cache(world: &RouteCacheWorld) {
    if world.is_skipped() {
        return;
    }
    world
        .latest_put_result
        .get()
        .expect("put result should be present")
        .clone()
        .expect("put should succeed");

    let loaded = world
        .first_loaded_plan
        .get()
        .expect("get result should be present")
        .clone()
        .expect("get should succeed");

    assert_eq!(loaded, Some(TestPlan::new("req-happy", 42)));
}

#[then("the cache reports a miss")]
fn the_cache_reports_a_miss(world: &RouteCacheWorld) {
    if world.is_skipped() {
        return;
    }
    let loaded = world
        .first_loaded_plan
        .get()
        .expect("get result should be present")
        .clone()
        .expect("get should succeed");

    assert_eq!(loaded, None);
}

#[then("the error maps to a serialization failure")]
fn the_error_maps_to_a_serialization_failure(world: &RouteCacheWorld) {
    if world.is_skipped() {
        return;
    }
    let error = world.latest_error.get().expect("error should be recorded");
    assert!(matches!(error, RouteCacheError::Serialization { .. }));
}

#[then("the error maps to a backend failure")]
fn the_error_maps_to_a_backend_failure(world: &RouteCacheWorld) {
    if world.is_skipped() {
        return;
    }
    let error = world.latest_error.get().expect("error should be recorded");
    assert!(matches!(error, RouteCacheError::Backend { .. }));
}

#[then("each cache key keeps its own plan")]
fn each_cache_key_keeps_its_own_plan(world: &RouteCacheWorld) {
    if world.is_skipped() {
        return;
    }
    world
        .latest_put_result
        .get()
        .expect("latest put result should be present")
        .clone()
        .expect("second put should succeed");

    let first_loaded = world
        .first_loaded_plan
        .get()
        .expect("first get result should be present")
        .clone()
        .expect("first get should succeed");
    let second_loaded = world
        .second_loaded_plan
        .get()
        .expect("second get result should be present")
        .clone()
        .expect("second get should succeed");

    assert_eq!(first_loaded, Some(TestPlan::new("req-first", 1)));
    assert_eq!(second_loaded, Some(TestPlan::new("req-second", 2)));
}

#[scenario(
    path = "tests/features/route_cache_redis.feature",
    name = "Stored plans round-trip through Redis"
)]
fn stored_plans_round_trip_through_redis(world: RouteCacheWorld) {
    drop(world);
}

#[scenario(
    path = "tests/features/route_cache_redis.feature",
    name = "Missing keys return a cache miss"
)]
fn missing_keys_return_a_cache_miss(world: RouteCacheWorld) {
    drop(world);
}

#[scenario(
    path = "tests/features/route_cache_redis.feature",
    name = "Malformed cached bytes surface as serialization failures"
)]
fn malformed_cached_bytes_surface_as_serialization_failures(world: RouteCacheWorld) {
    drop(world);
}

#[scenario(
    path = "tests/features/route_cache_redis.feature",
    name = "Unreachable Redis surfaces as a backend failure"
)]
fn unreachable_redis_surfaces_as_a_backend_failure(world: RouteCacheWorld) {
    drop(world);
}

#[scenario(
    path = "tests/features/route_cache_redis.feature",
    name = "Distinct cache keys do not overwrite each other"
)]
fn distinct_cache_keys_do_not_overwrite_each_other(world: RouteCacheWorld) {
    drop(world);
}
