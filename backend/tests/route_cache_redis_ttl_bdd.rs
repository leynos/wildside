//! TTL and jitter behavioural tests for the Redis-backed `RouteCache` adapter.
//!
//! This module contains behavioural tests verifying TTL (time-to-live) and jitter
//! functionality. Live-Redis scenarios require a `redis-server` binary on `PATH`.
//! When the binary is absent or `SKIP_REDIS_TESTS=1` is set, scenarios are
//! skipped at runtime.

use std::sync::Arc;

use backend::{
    domain::ports::{RouteCache, RouteCacheKey},
    outbound::cache::RedisRouteCache,
};
use bb8_redis::redis::AsyncCommands;
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};
use serde::{Deserialize, Serialize};
use tokio::runtime::Runtime;

mod support;

use support::{redis::RedisTestServer, should_skip_redis_tests};

#[derive(Clone)]
struct RuntimeHandle(Arc<Runtime>);

#[derive(Clone)]
struct RedisServerHandle(Arc<RedisTestServer>);

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
struct TtlJitterWorld {
    runtime: Slot<RuntimeHandle>,
    server: Slot<RedisServerHandle>,
    cache: Slot<CacheHandle>,
    skip_reason: Slot<String>,
    has_printed_skip_message: Slot<bool>,
    ttl_records: Slot<Vec<Option<usize>>>,
}

impl TtlJitterWorld {
    fn runtime(&self) -> RuntimeHandle {
        self.runtime.get().expect("runtime should be initialized")
    }

    fn cache(&self) -> CacheHandle {
        self.cache.get().expect("cache should be initialized")
    }

    fn is_skipped(&self) -> bool {
        if let Some(reason) = self.skip_reason.get() {
            // Only print skip message once per scenario
            if self.has_printed_skip_message.get() != Some(true) {
                eprintln!("SKIP-REDIS-TESTS: scenario skipped ({reason})");
                self.has_printed_skip_message.set(true);
            }
            true
        } else {
            false
        }
    }

    /// Bootstrap a live Redis connection for scenarios that need it.
    ///
    /// Checks skip conditions, starts a RedisTestServer, creates the pool and
    /// cache, and stores all handles into the world state. Returns `true` if
    /// the scenario should be skipped (Redis unavailable or SKIP_REDIS_TESTS set).
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
        let cache = RedisRouteCache::new(pool);

        self.runtime.set(RuntimeHandle(Arc::new(runtime)));
        self.server.set(RedisServerHandle(Arc::new(server)));
        self.cache.set(CacheHandle(Arc::new(cache)));

        false
    }

    fn store_plan(&self, key: &str, plan: TestPlan) {
        let key = RouteCacheKey::new(key).expect("valid key");
        let runtime = self.runtime();
        let cache = self.cache();
        runtime
            .0
            .block_on(async move { cache.0.put(&key, &plan).await })
            .expect("put should succeed");
    }

    fn query_ttl(&self, key: &str) -> Option<usize> {
        let server = self
            .server
            .get()
            .expect("redis server should be initialized");
        let runtime = self.runtime();
        runtime.0.block_on(async {
            let pool = server.0.pool().await.expect("get pool");
            let mut conn = pool.get().await.expect("get connection");
            conn.ttl::<_, isize>(key)
                .await
                .ok()
                .and_then(|ttl| if ttl > 0 { Some(ttl as usize) } else { None })
        })
    }

    fn record_ttls_for_keys(&self, keys: &[&str]) {
        let ttls: Vec<Option<usize>> = keys.iter().map(|key| self.query_ttl(key)).collect();
        self.ttl_records.set(ttls);
    }
}

#[fixture]
fn world() -> TtlJitterWorld {
    TtlJitterWorld::default()
}

#[given("a running Redis-backed route cache")]
fn a_running_redis_backed_route_cache(world: &TtlJitterWorld) {
    if world.bootstrap_redis() {
        return;
    }
}

#[when("five plans are stored under distinct cache keys")]
fn five_plans_are_stored_under_distinct_cache_keys(world: &TtlJitterWorld) {
    if world.is_skipped() {
        return;
    }
    for i in 1..=5 {
        let key = format!("route:jitter-{i}");
        world.store_plan(&key, TestPlan::new(&format!("req-{i}"), i));
    }
    // Record TTLs for all five keys
    world.record_ttls_for_keys(&[
        "route:jitter-1",
        "route:jitter-2",
        "route:jitter-3",
        "route:jitter-4",
        "route:jitter-5",
    ]);
}

#[then("not all recorded TTLs are identical")]
fn not_all_recorded_ttls_are_identical(world: &TtlJitterWorld) {
    if world.is_skipped() {
        return;
    }
    let ttls = world
        .ttl_records
        .get()
        .expect("TTL records should be present");

    // All TTLs should be Some (keys should exist)
    assert!(
        ttls.iter().all(|t| t.is_some()),
        "All keys should have TTLs"
    );

    // Extract the actual TTL values
    let ttl_values: Vec<usize> = ttls.iter().filter_map(|t| *t).collect();

    // Check that all TTLs are within the expected range (77,760 to 95,040 seconds)
    // This is 24 hours +/- 10%
    const MIN_TTL: usize = 77_760;
    const MAX_TTL: usize = 95_040;
    assert!(
        ttl_values
            .iter()
            .all(|&ttl| (MIN_TTL..=MAX_TTL).contains(&ttl)),
        "All TTLs should be within the jittered range [{MIN_TTL}, {MAX_TTL}], got {ttl_values:?}"
    );

    // Check that we have variation by computing the range
    let min_ttl = *ttl_values.iter().min().expect("should have TTLs");
    let max_ttl = *ttl_values.iter().max().expect("should have TTLs");
    let ttl_range = max_ttl - min_ttl;

    // We expect at least 1000 seconds of variation across 5 samples (very conservative)
    // This is much less than the theoretical maximum range of ~17,280 seconds
    assert!(
        ttl_range >= 1000,
        "TTL range should be at least 1000 seconds to demonstrate jitter, got {ttl_range} seconds (min={min_ttl}, max={max_ttl})"
    );
}

#[scenario(
    path = "tests/features/route_cache_redis.feature",
    name = "Jittered writes produce varying TTLs"
)]
fn jittered_writes_produce_varying_ttls(world: TtlJitterWorld) {
    drop(world);
}
