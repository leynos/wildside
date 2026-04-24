//! Behavioural tests for route cache key canonicalization with the Redis-backed
//! `RouteCache` adapter.
//!
//! These scenarios require a live `redis-server` binary on `PATH`; when absent
//! or `SKIP_REDIS_TESTS=1` is set, they are skipped at runtime.

use std::sync::Arc;

use backend::{
    domain::ports::{RouteCache, RouteCacheError, RouteCacheKey},
    outbound::cache::RedisRouteCache,
};
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::runtime::Runtime;

mod support;

use support::{redis::RedisTestServer, should_skip_redis_tests};

#[derive(Clone)]
struct RuntimeHandle(Arc<Runtime>);

#[derive(Clone)]
struct CacheHandle(Arc<RedisRouteCache<TestPlan>>);

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
struct RouteCacheKeyWorld {
    runtime: Slot<RuntimeHandle>,
    server: Slot<Arc<RedisTestServer>>,
    cache: Slot<CacheHandle>,
    first_key: Slot<RouteCacheKey>,
    second_key: Slot<RouteCacheKey>,
    loaded_plan: Slot<Result<Option<TestPlan>, RouteCacheError>>,
    skip_reason: Slot<String>,
    has_printed_skip_message: Slot<bool>,
}

impl RouteCacheKeyWorld {
    fn runtime(&self) -> RuntimeHandle {
        self.runtime.get().expect("runtime should be initialized")
    }

    fn cache(&self) -> CacheHandle {
        self.cache.get().expect("cache should be initialized")
    }

    fn is_skipped(&self) -> bool {
        if let Some(reason) = self.skip_reason.get() {
            if self.has_printed_skip_message.get() != Some(true) {
                eprintln!("SKIP-REDIS-TESTS: scenario skipped ({reason})");
                self.has_printed_skip_message.set(true);
            }
            true
        } else {
            false
        }
    }

    fn bootstrap_redis(&self) -> bool {
        if should_skip_redis_tests() {
            self.skip_reason
                .set("redis-server unavailable or SKIP_REDIS_TESTS set".to_owned());
            return true;
        }

        let runtime = Runtime::new().expect("create runtime");
        let server = runtime
            .block_on(async { RedisTestServer::start().await })
            .expect("start redis-server");
        let pool = runtime
            .block_on(async { server.pool().await })
            .expect("create redis pool");

        self.runtime.set(RuntimeHandle(Arc::new(runtime)));
        self.server.set(Arc::new(server));
        self.cache
            .set(CacheHandle(Arc::new(RedisRouteCache::new(pool))));

        false
    }

    fn derive_equivalent_keys(&self) {
        let first_payload = json!({
            "destination": {
                "lng": -0.1234561,
                "lat": 51.5000001
            },
            "origin": {
                "lat": 51.4999999,
                "lng": -0.1234564
            },
            "preferences": {
                "interestThemeIds": ["theme-c", "theme-a", "theme-b"],
                "avoidStairs": true
            }
        });
        let second_payload = json!({
            "preferences": {
                "avoidStairs": true,
                "interestThemeIds": ["theme-b", "theme-c", "theme-a"]
            },
            "origin": {
                "lng": -0.1234562,
                "lat": 51.5000001
            },
            "destination": {
                "lat": 51.4999998,
                "lng": -0.12345649
            }
        });

        self.first_key
            .set(RouteCacheKey::for_route_request(&first_payload).expect("first route key"));
        self.second_key
            .set(RouteCacheKey::for_route_request(&second_payload).expect("second route key"));
    }

    fn store_plan_under_first_key(&self, plan: TestPlan) {
        let key = self
            .first_key
            .get()
            .expect("first key should be initialized")
            .clone();
        let runtime = self.runtime();
        let cache = self.cache();

        runtime
            .0
            .block_on(async move { cache.0.put(&key, &plan).await })
            .expect("put should succeed");
    }

    fn load_plan_with_second_key(&self) {
        let key = self
            .second_key
            .get()
            .expect("second key should be initialized")
            .clone();
        let runtime = self.runtime();
        let cache = self.cache();
        let result = runtime.0.block_on(async move { cache.0.get(&key).await });
        self.loaded_plan.set(result);
    }
}

#[fixture]
fn world() -> RouteCacheKeyWorld {
    RouteCacheKeyWorld::default()
}

#[given("a running Redis-backed route cache")]
fn a_running_redis_backed_route_cache(world: &RouteCacheKeyWorld) {
    if world.bootstrap_redis() {
        return;
    }
}

#[when("semantically equivalent route requests are canonicalized into cache keys")]
fn semantically_equivalent_route_requests_are_canonicalized_into_cache_keys(
    world: &RouteCacheKeyWorld,
) {
    if world.is_skipped() {
        return;
    }
    world.derive_equivalent_keys();
}

#[when("a plan is stored under the first canonical cache key")]
fn a_plan_is_stored_under_the_first_canonical_cache_key(world: &RouteCacheKeyWorld) {
    if world.is_skipped() {
        return;
    }
    world.store_plan_under_first_key(TestPlan::new("req-canonical", 73));
}

#[when("the cache is read with the second canonical cache key")]
fn the_cache_is_read_with_the_second_canonical_cache_key(world: &RouteCacheKeyWorld) {
    if world.is_skipped() {
        return;
    }
    world.load_plan_with_second_key();
}

#[then("both route requests share the same canonical cache key")]
fn both_route_requests_share_the_same_canonical_cache_key(world: &RouteCacheKeyWorld) {
    if world.is_skipped() {
        return;
    }
    assert_eq!(world.first_key.get(), world.second_key.get());
}

#[then("the canonical cache key uses the route v1 sha256 format")]
fn the_canonical_cache_key_uses_the_route_v1_sha256_format(world: &RouteCacheKeyWorld) {
    if world.is_skipped() {
        return;
    }
    let key = world
        .first_key
        .get()
        .expect("first key should be initialized");
    let digest = key
        .as_str()
        .strip_prefix("route:v1:")
        .expect("route:v1 prefix should be present");

    assert_eq!(digest.len(), 64);
    assert!(
        digest
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
    );
}

#[then("the same plan is returned from the cache")]
fn the_same_plan_is_returned_from_the_cache(world: &RouteCacheKeyWorld) {
    if world.is_skipped() {
        return;
    }
    let loaded_plan = world
        .loaded_plan
        .get()
        .expect("loaded plan should be recorded");
    assert_eq!(
        loaded_plan.as_ref().expect("get should succeed"),
        &Some(TestPlan::new("req-canonical", 73))
    );
}

#[scenario(
    path = "tests/features/route_cache_key_canonicalization.feature",
    name = "Semantically equivalent route requests share one Redis cache slot"
)]
fn semantically_equivalent_route_requests_share_one_redis_cache_slot(world: RouteCacheKeyWorld) {
    drop(world);
}
