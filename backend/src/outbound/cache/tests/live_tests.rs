//! Live Redis tests for the route cache adapter.
//!
//! These tests require a running `redis-server` binary and are marked with
//! `#[ignore]` to keep the default test suite fast and self-contained.
//!
//! Run these tests explicitly with:
//! ```sh
//! cargo test -- --ignored
//! ```

use bb8_redis::{RedisConnectionManager, bb8::Pool, redis::cmd};

use crate::domain::ports::{RouteCache, RouteCacheError, RouteCacheKey};
use crate::outbound::cache::RedisRouteCache;
use crate::outbound::cache::test_helpers::{TestPlan, assert_put_get_round_trips_with_plan};
use crate::test_support::redis::{RedisTestServer, unused_redis_url};

/// Starts a test Redis server and returns a connection pool.
async fn start_test_redis() -> (RedisTestServer, Pool<RedisConnectionManager>) {
    let server = RedisTestServer::start().await.expect("start redis-server");
    let pool = server.pool().await.expect("create redis pool");
    (server, pool)
}

#[tokio::test]
#[ignore = "requires redis-server binary; run with `cargo test -- --ignored`"]
async fn live_get_returns_none_for_missing_key() {
    let (_server, pool) = start_test_redis().await;
    let cache = RedisRouteCache::<TestPlan>::new(pool);
    let key = RouteCacheKey::new("route:missing").expect("valid key");

    let result = cache.get(&key).await.expect("missing-key lookup succeeds");

    assert_eq!(result, None);
}

#[tokio::test]
#[ignore = "requires redis-server binary; run with `cargo test -- --ignored`"]
async fn live_put_followed_by_get_round_trips_the_typed_plan() {
    let (_server, pool) = start_test_redis().await;
    let cache = RedisRouteCache::<TestPlan>::new(pool);
    let plan = TestPlan::new("req-1", 42);

    assert_put_get_round_trips_with_plan(&cache, &plan)
        .await
        .expect("put/get round-trip succeeds");
}

#[tokio::test]
#[ignore = "requires redis-server binary; run with `cargo test -- --ignored`"]
async fn live_corrupted_cached_bytes_map_to_serialization_errors() {
    let (_server, pool) = start_test_redis().await;
    let cache = RedisRouteCache::<TestPlan>::new(pool.clone());
    let key = RouteCacheKey::new("route:corrupt").expect("valid key");
    let mut connection = pool.get().await.expect("redis connection");

    cmd("SET")
        .arg(key.as_str())
        .arg(vec![0_u8, 159, 146, 150])
        .query_async::<()>(&mut *connection)
        .await
        .expect("seed corrupt bytes");

    let result = cache
        .get(&key)
        .await
        .expect_err("corrupt payload should fail");

    assert!(matches!(result, RouteCacheError::Serialization { .. }));
}

#[tokio::test]
async fn command_failures_map_to_backend_errors() {
    let unreachable_url = unused_redis_url().await.expect("unused redis url");
    let manager = RedisConnectionManager::new(unreachable_url.as_str()).expect("redis manager");
    let pool = Pool::builder().max_size(1).build_unchecked(manager);
    let cache = RedisRouteCache::<TestPlan>::new(pool);
    let key = RouteCacheKey::new("route:backend").expect("valid key");

    let result = cache
        .get(&key)
        .await
        .expect_err("unreachable backend should fail");

    assert!(matches!(result, RouteCacheError::Backend { .. }));
}
