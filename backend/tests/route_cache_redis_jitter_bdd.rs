//! Behavioural tests for jitter-related TTL behaviour in the Redis-backed
//! `RouteCache` adapter.
//!
//! Extracted from `route_cache_redis_bdd.rs` to keep each file under
//! 400 lines. These scenarios require a live `redis-server` binary on
//! `PATH`; when absent or `SKIP_REDIS_TESTS=1` is set, they are skipped
//! at runtime.

use std::sync::Arc;

use backend::{
    domain::ports::{RouteCache, RouteCacheError, RouteCacheKey},
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
struct JitterWorld {
    runtime: Slot<RuntimeHandle>,
    server: Slot<RedisServerHandle>,
    cache: Slot<CacheHandle>,
    latest_put_result: Slot<Result<(), RouteCacheError>>,
    ttl_records: Slot<Vec<Option<usize>>>,
    stored_keys: Slot<Vec<String>>,
    skip_reason: Slot<String>,
    has_printed_skip_message: Slot<bool>,
}

impl JitterWorld {
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
        let result = runtime
            .0
            .block_on(async move { cache.0.put(&key, &plan).await });
        self.latest_put_result.set(result);
    }

    async fn query_ttl_async(
        conn: &mut impl AsyncCommands,
        key: &str,
    ) -> Result<Option<usize>, bb8_redis::redis::RedisError> {
        let ttl: i64 = conn.ttl(key).await?;
        Ok(if ttl > 0 { Some(ttl as usize) } else { None })
    }

    fn record_ttls_for_keys(&self, keys: &[String]) {
        let server = self.server.get().expect("redis server initialized");
        let runtime = self.runtime();
        let ttls = runtime.0.block_on(async {
            let mut conn = server.0.raw_connection().await.expect("raw redis conn");
            let mut out = Vec::new();
            for key in keys {
                out.push(
                    Self::query_ttl_async(&mut conn, key)
                        .await
                        .expect("TTL query should not fail"),
                );
            }
            out
        });
        self.ttl_records.set(ttls);
    }
}

#[fixture]
fn world() -> JitterWorld {
    JitterWorld::default()
}

#[given("a running Redis-backed route cache")]
fn a_running_redis_backed_route_cache(world: &JitterWorld) {
    if world.bootstrap_redis() {
        return;
    }
}

#[when("five plans are stored under distinct cache keys")]
fn five_plans_are_stored_under_distinct_cache_keys(world: &JitterWorld) {
    if world.is_skipped() {
        return;
    }
    let keys: Vec<String> = (0..5).map(|i| format!("route:jitter:{i}")).collect();
    for (i, key) in keys.iter().enumerate() {
        world.store_plan(key, TestPlan::new(&format!("req-jitter-{i}"), i as u64));
        world
            .latest_put_result
            .get()
            .expect("put result present")
            .clone()
            .expect("put should succeed");
    }
    world.record_ttls_for_keys(&keys);
    world.stored_keys.set(keys);
}

#[then("not all recorded TTLs are identical")]
fn not_all_recorded_ttls_are_identical(world: &JitterWorld) {
    if world.is_skipped() {
        return;
    }
    let ttls = world.ttl_records.get().expect("TTLs recorded");
    let all_some: Vec<usize> = ttls
        .into_iter()
        .map(|t| t.expect("TTL should be set (Some)"))
        .collect();
    let first = all_some[0];
    assert!(
        all_some.iter().any(|&t| t != first),
        "expected TTLs to vary due to jitter, but all were {first}s"
    );
}

#[then("all recorded TTLs fall within the configured jitter window")]
fn all_recorded_ttls_fall_within_the_configured_jitter_window(world: &JitterWorld) {
    if world.is_skipped() {
        return;
    }
    use backend::outbound::cache::{DEFAULT_BASE_TTL_SECS, DEFAULT_JITTER_FRACTION};
    let jitter = (DEFAULT_BASE_TTL_SECS as f64 * DEFAULT_JITTER_FRACTION) as usize;
    // Allow a small slack for Redis countdown drift between the SET and
    // the subsequent TTL query.
    let drift_slack_secs: usize = 2;
    let lower = DEFAULT_BASE_TTL_SECS as usize - jitter - drift_slack_secs;
    let upper = DEFAULT_BASE_TTL_SECS as usize + jitter;
    let ttls = world.ttl_records.get().expect("TTLs recorded");
    for ttl in ttls {
        let t = ttl.expect("TTL should be Some");
        assert!(
            t >= lower && t <= upper,
            "TTL {t}s is outside expected window [{lower}s, {upper}s]"
        );
    }
}

#[scenario(
    path = "tests/features/route_cache_redis.feature",
    name = "Jittered writes produce varying TTLs"
)]
fn jittered_writes_produce_varying_ttls(world: JitterWorld) {
    drop(world);
}
