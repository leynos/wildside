//! The shared embedded cluster, its pool, and worker registration.
//!
//! Every PostgreSQL test binary shares one embedded cluster, so the canaries
//! build one environment for the whole binary and borrow its runtime. The
//! worker registration helpers live here as well, because a row's lock owner
//! and its heartbeat are the same database fact.

use std::sync::OnceLock;
use std::time::Duration;

use apalis_core::backend::Backend;
use apalis_core::worker::context::WorkerContext;
use apalis_postgres::{Config, PostgresStorage};
use futures::StreamExt;
use pg_embedded_setup_unpriv::TemporaryDatabase;
use serde_json::Value;
use sqlx::PgPool;
use tokio::runtime::Runtime;

use crate::support::atexit_cleanup::{bootstrap_shared_cluster, ensure_stable_cluster_environment};
use crate::support::embedded_postgres::provision_template_database;

pub(crate) struct CanaryEnvironment {
    pub(crate) runtime: Runtime,
    pub(crate) pool: PgPool,
    // Dropping this removes the canary database, so it is held for the process
    // lifetime alongside the shared cluster.
    _database: TemporaryDatabase,
}

impl CanaryEnvironment {
    pub(crate) fn shared() -> &'static Self {
        static ENVIRONMENT: OnceLock<CanaryEnvironment> = OnceLock::new();
        ENVIRONMENT.get_or_init(|| {
            ensure_stable_cluster_environment().expect("stable PostgreSQL environment");
            let runtime = Runtime::new().expect("create canary Tokio runtime");
            let cluster = bootstrap_shared_cluster().expect("start embedded PostgreSQL");
            let database = provision_template_database(cluster).expect("provision canary database");
            let url = database.url().to_string();
            let pool = canary_pool(&runtime, &url);
            Self {
                runtime,
                pool,
                _database: database,
            }
        })
    }

    pub(crate) fn run<F: std::future::Future>(&self, future: F) -> F::Output {
        self.runtime.block_on(future)
    }
}

/// Build the canary pool with a generous acquire timeout.
///
/// Every PostgreSQL test binary shares one embedded cluster, so the server can
/// be saturated by unrelated suites while a canary waits for a connection. The
/// default 30-second acquire timeout then expires and the canary fails for a
/// reason unrelated to the contract it pins. Waiting longer, and holding fewer
/// connections, keeps the canaries deterministic without changing what they
/// assert.
///
/// The pool must only ever be driven from [`CanaryEnvironment::runtime`].
/// `sqlx` ties a connection to the runtime that created it, and dropping that
/// runtime while the connection is checked out leaks the pool permit. Enough
/// leaked permits exhaust the pool permanently, and every later acquire waits
/// out the timeout. A helper that needs the pool therefore borrows the shared
/// runtime instead of building its own.
fn canary_pool(runtime: &Runtime, url: &str) -> PgPool {
    runtime.block_on(async {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(4)
            .acquire_timeout(Duration::from_secs(120))
            .connect(url)
            .await
            .expect("connect canary pool");
        PostgresStorage::<(), (), ()>::setup(&pool)
            .await
            .expect("set up Apalis storage");
        pool
    })
}

/// Register the canary worker so `lock_by` satisfies its foreign key.
pub(crate) async fn register_worker(pool: &PgPool, worker: &str) {
    sqlx::query(
        "INSERT INTO apalis.workers (id, worker_type, storage_name)
         VALUES ($1, 'canary', 'canary')
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(worker)
    .execute(pool)
    .await
    .expect("register canary worker");
}

/// Register a worker under the queue name its rows belong to.
///
/// Apalis's orphan recovery only returns rows whose owner's `worker_type`
/// equals the queue, so a canary that wants recovery to see its crashed worker
/// must register it with that type rather than the generic one
/// [`register_worker`] uses.
pub(crate) async fn register_queue_worker(pool: &PgPool, worker: &str, queue: &str) {
    sqlx::query(
        "INSERT INTO apalis.workers (id, worker_type, storage_name)
         VALUES ($1, $2, 'canary')
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(worker)
    .bind(queue)
    .execute(pool)
    .await
    .expect("register canary queue worker");
}

/// Backdate a worker's heartbeat so orphan recovery treats its rows as stale.
pub(crate) async fn mark_worker_stale(pool: &PgPool, worker: &str) {
    sqlx::query("UPDATE apalis.workers SET last_seen = now() - interval '1 hour' WHERE id = $1")
        .bind(worker)
        .execute(pool)
        .await
        .expect("backdate the canary worker heartbeat");
}

/// How long orphan recovery gets to return a row to the fetcher.
const RECOVERY_WINDOW: Duration = Duration::from_secs(10);

/// Return a stale row to the fetcher through Apalis's own recovery stream.
///
/// The recovery predicate runs from the backend heartbeat, so this polls the
/// real stream rather than a copy of its SQL. A sweeper under a distinct name
/// drives the heartbeat, because polling under the crashed worker's name would
/// refresh the heartbeat that makes its row orphaned.
pub(crate) fn recover_orphaned_row(
    environment: &CanaryEnvironment,
    queue: &str,
    sweeper: &str,
    id: &str,
) {
    environment.run(async {
        // `keep_alive` updates the sweeper's own row and matches on the
        // worker type, so the heartbeat fails with `WORKER_DOES_NOT_EXIST`
        // unless the sweeper is registered under the queue first.
        register_queue_worker(&environment.pool, sweeper, queue).await;
        let config = Config::new(queue)
            .set_keep_alive(Duration::from_millis(50))
            .set_reenqueue_orphaned_after(Duration::from_secs(30));
        let storage = PostgresStorage::<Value>::new_with_config(&environment.pool, &config);
        let beat = Backend::heartbeat(&storage, &WorkerContext::new::<Value>(sweeper));
        futures::pin_mut!(beat);

        let recovered = tokio::time::timeout(RECOVERY_WINDOW, async {
            // The row starts `Running`, so checking before the first beat
            // skips nothing and keeps the loop body free of a nested branch.
            while row_status(&environment.pool, id).await != "Pending" {
                beat.next()
                    .await
                    .expect("the heartbeat stream stays open")
                    .expect("the recovery sweep succeeds");
            }
        })
        .await;
        recovered.expect("orphan recovery returns the row within its window");
    });
}

/// Read one row's persisted status.
async fn row_status(pool: &PgPool, id: &str) -> String {
    sqlx::query_scalar("SELECT status FROM apalis.jobs WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("read the recovered row")
}
