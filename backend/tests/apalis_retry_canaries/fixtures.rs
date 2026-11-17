//! Shared PostgreSQL fixtures for the Apalis retry canaries.
//!
//! Every canary pins one behaviour of `apalis-postgres` rc.8 against a real
//! database. This module owns the environment, the row builders, and the two
//! fetch helpers, so each canary module reads as an assertion about the
//! contract rather than as database setup.
//!
//! The two fetch helpers answer different questions. [`fetch_one_task`] drives
//! the real backend stream and is the only way to see what a delivery carries,
//! but a window that expires cannot distinguish an empty poll from one that
//! never completed. [`claim_due_rows`] calls `apalis.get_jobs` directly, so it
//! completes or errors, and a count of zero is evidence about the predicate.

use std::sync::OnceLock;
use std::time::Duration;

use apalis_core::backend::Backend;
use apalis_core::task::Task;
use apalis_core::task::attempt::Attempt;
use apalis_core::worker::context::WorkerContext;
use apalis_postgres::{Config, PgContext, PgTask, PostgresStorage};
use futures::StreamExt;
use pg_embedded_setup_unpriv::TemporaryDatabase;
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::runtime::Runtime;

use super::support::atexit_cleanup::{bootstrap_shared_cluster, ensure_stable_cluster_environment};
use super::support::embedded_postgres::provision_template_database;

/// How long to watch the fetcher before concluding a delivery is not coming.
const FETCH_WINDOW: Duration = Duration::from_millis(1_500);

pub(super) struct CanaryEnvironment {
    pub(super) runtime: Runtime,
    pub(super) pool: PgPool,
    // Dropping this removes the canary database, so it is held for the process
    // lifetime alongside the shared cluster.
    _database: TemporaryDatabase,
}

impl CanaryEnvironment {
    pub(super) fn shared() -> &'static Self {
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

    pub(super) fn run<F: std::future::Future>(&self, future: F) -> F::Output {
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
pub(super) async fn register_worker(pool: &PgPool, worker: &str) {
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

/// Scheduling state for one canary job row.
///
/// The defaults describe the row the wrapper most often has to reason about:
/// a `Failed` job that is due now, has spent one of its five attempts, and
/// carries no metadata or worker lock. Cases adjust only the field they are
/// about.
pub(super) struct JobRow<'a> {
    queue: &'a str,
    id: &'a str,
    status: &'a str,
    attempts: i32,
    max_attempts: i32,
    run_at_offset_seconds: f64,
    metadata: Option<Value>,
    locked_by: Option<&'a str>,
}

impl<'a> JobRow<'a> {
    pub(super) fn new(queue: &'a str, id: &'a str, status: &'a str) -> Self {
        Self {
            queue,
            id,
            status,
            attempts: 1,
            max_attempts: 5,
            run_at_offset_seconds: -1.0,
            metadata: None,
            locked_by: None,
        }
    }

    pub(super) fn attempts(mut self, attempts: i32) -> Self {
        self.attempts = attempts;
        self
    }

    pub(super) fn max_attempts(mut self, max_attempts: i32) -> Self {
        self.max_attempts = max_attempts;
        self
    }

    /// Offset the row's `run_at` from now, in seconds; negative is already due.
    pub(super) fn run_at_offset_seconds(mut self, seconds: f64) -> Self {
        self.run_at_offset_seconds = seconds;
        self
    }

    pub(super) fn metadata(mut self, metadata: Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub(super) fn locked_by(mut self, worker: &'a str) -> Self {
        self.locked_by = Some(worker);
        self
    }
}

/// Insert one job row with explicit scheduling state.
pub(super) async fn insert_job(pool: &PgPool, row: JobRow<'_>) {
    // `apalis.jobs.job` is `bytea` and `metadata` is `jsonb` after the upstream
    // migrations, so the bindings use those exact types rather than text that
    // PostgreSQL would have to reinterpret.
    let payload = json!({ "v": "v1", "probeId": row.id })
        .to_string()
        .into_bytes();
    sqlx::query(
        "INSERT INTO apalis.jobs
            (job, id, job_type, status, attempts, max_attempts, run_at, metadata, lock_by)
         VALUES ($1, $2, $3, $4, $5, $6, now() + make_interval(secs => $7), $8, $9)",
    )
    .bind(payload)
    .bind(row.id)
    .bind(row.queue)
    .bind(row.status)
    .bind(row.attempts)
    .bind(row.max_attempts)
    .bind(row.run_at_offset_seconds)
    .bind(row.metadata.map(sqlx::types::Json))
    .bind(row.locked_by)
    .execute(pool)
    .await
    .expect("insert canary job");
}

/// A fetched task reduced to the fields the wrapper reads.
#[derive(Debug)]
pub(super) struct FetchedTask {
    pub(super) task_id: String,
    pub(super) attempt: usize,
    pub(super) max_attempts: i32,
}

/// Poll the queue through the real Apalis backend until one task is delivered.
///
/// The stream yields one registration heartbeat (`Ok(None)`) before the first
/// fetch. A window that expires leaves the positive canary unproven rather
/// than failed, so it panics with that reading instead of reporting "not
/// fetched"; the negative canaries use [`claim_due_rows`], whose result they
/// can interpret.
pub(super) fn fetch_one_task(
    environment: &CanaryEnvironment,
    queue: &str,
    worker: &str,
) -> FetchedTask {
    describe(&fetch_task(environment, queue, worker))
}

/// Poll the queue through the real Apalis backend and return the delivery.
///
/// Unlike [`fetch_one_task`], this keeps every part of the fetched task, so a
/// canary can hand it to the production wrapper. It panics on a window that
/// expires for the reason given there.
pub(super) fn fetch_task(
    environment: &CanaryEnvironment,
    queue: &str,
    worker: &str,
) -> PgTask<Value> {
    environment.run(async {
        let storage =
            PostgresStorage::<Value>::new_with_config(&environment.pool, &Config::new(queue));
        let stream = Backend::poll(storage, &WorkerContext::new::<Value>(worker));
        futures::pin_mut!(stream);

        loop {
            match tokio::time::timeout(FETCH_WINDOW, stream.next()).await {
                Ok(Some(Ok(Some(task)))) => return task,
                Ok(Some(Ok(None))) => {}
                Ok(Some(Err(error))) => panic!("fetch failed: {error}"),
                Ok(None) => panic!("the fetch stream ended before delivering a task"),
                Err(_) => panic!(
                    "the fetch did not complete within {FETCH_WINDOW:?}; \
                     the canary is inconclusive"
                ),
            }
        }
    })
}

/// Claim due rows through `apalis.get_jobs` and report how many were claimed.
///
/// Calling the function directly either completes or errors, so the returned
/// count is evidence about the fetch predicate. The backend stream cannot
/// offer that: an expired observation window and an empty poll look the same.
pub(super) fn claim_due_rows(environment: &CanaryEnvironment, queue: &str, worker: &str) -> i64 {
    environment.run(async {
        sqlx::query_scalar("SELECT count(*) FROM apalis.get_jobs($1, $2, $3)")
            .bind(worker)
            .bind(queue)
            .bind(1_i32)
            .fetch_one(&environment.pool)
            .await
            .expect("a completed fetch reports how many rows it claimed")
    })
}

/// Persisted scheduling state the fetcher would change on a successful lock.
///
/// `apalis.get_jobs` queues the row by setting `status = 'Queued'` and records
/// the worker in `lock_by`. Promotion to `'Running'` is a second step
/// (`lock_by_id.sql`) that only the worker pipeline performs, so the fetcher
/// alone leaves it `'Queued'`. An unchanged row proves the fetcher never
/// claimed it, which makes the negative canaries independent of how long the
/// observation window happens to last.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct RowState {
    pub(super) status: String,
    pub(super) attempts: i32,
    pub(super) lock_by: Option<String>,
}

pub(super) fn read_state(runtime: &Runtime, pool: &PgPool, id: &str) -> RowState {
    runtime.block_on(async {
        let row: (String, i32, Option<String>) =
            sqlx::query_as("SELECT status, attempts, lock_by FROM apalis.jobs WHERE id = $1")
                .bind(id)
                .fetch_one(pool)
                .await
                .expect("read canary job state");
        RowState {
            status: row.0,
            attempts: row.1,
            lock_by: row.2,
        }
    })
}

/// A running row locked by a canary worker, as the wrapper would see it.
///
/// The attempt and its limit travel in the task rather than being read from the
/// row, because that is where Apalis takes them from when it acknowledges a
/// delivery: it decides the status from the context it cloned before the
/// handler ran.
pub(super) fn locked_task(
    id: &str,
    worker: &str,
    max_attempts: i32,
    attempt: usize,
) -> PgTask<Value> {
    Task::builder(json!({ "v": "v1", "probeId": id }))
        .with_ctx(
            PgContext::new()
                .with_max_attempts(max_attempts)
                .with_lock_by(Some(worker.to_owned())),
        )
        .with_task_id(id.parse().expect("canary task id"))
        .with_attempt(Attempt::new_with_value(attempt))
        .build()
}

fn describe(task: &PgTask<Value>) -> FetchedTask {
    FetchedTask {
        task_id: task
            .parts
            .task_id
            .as_ref()
            .map(ToString::to_string)
            .expect("fetched task has an id"),
        attempt: task.parts.attempt.current(),
        max_attempts: task.parts.ctx.max_attempts(),
    }
}
