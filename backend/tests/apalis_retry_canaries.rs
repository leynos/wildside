//! Version-coupling canaries for durable retry scheduling on Apalis PostgreSQL.
//!
//! The retry wrapper depends on behaviour owned by `apalis-postgres` and its
//! `apalis.get_jobs` SQL function. Those are implementation details of a
//! release candidate, so this suite pins the behaviour the wrapper relies on:
//!
//! - the scheduling columns exist and keep their names;
//! - only due `Failed` rows with spare attempts are fetched;
//! - `Killed` rows and rows at their attempt cap are never fetched;
//! - a fetched task keeps its persisted attempt count;
//! - acknowledgement leaves `run_at`, `max_attempts`, and `metadata` intact;
//!   and
//! - the retry wrapper writes nothing unless the recorded lock owner matches
//!   the worker that delivered the task.
//!
//! A failure here means an Apalis upgrade changed the contract, not that the
//! wrapper regressed.

use std::sync::OnceLock;
use std::time::Duration;

use apalis_core::backend::Backend;
use apalis_core::worker::context::WorkerContext;
use apalis_postgres::{Config, PgTask, PostgresStorage};
use futures::StreamExt;
use pg_embedded_setup_unpriv::TemporaryDatabase;
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::runtime::Runtime;

include!("support/entrypoint.rs");
declare_test_support!(apalis_job_id, atexit_cleanup, embedded_postgres);

#[path = "apalis_retry_canaries/ack.rs"]
mod ack;
#[path = "apalis_retry_canaries/identity.rs"]
mod identity;
#[path = "apalis_retry_canaries/scheduler.rs"]
mod scheduler;

use support::apalis_job_id::apalis_job_id;
use support::atexit_cleanup::{bootstrap_shared_cluster, ensure_stable_cluster_environment};
use support::embedded_postgres::provision_template_database;

/// How long to watch the fetcher before concluding a row is not due.
const FETCH_WINDOW: Duration = Duration::from_millis(1_500);

struct CanaryEnvironment {
    runtime: Runtime,
    pool: PgPool,
    // Dropping this removes the canary database, so it is held for the process
    // lifetime alongside the shared cluster.
    _database: TemporaryDatabase,
}

impl CanaryEnvironment {
    fn shared() -> &'static Self {
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

    fn run<F: std::future::Future>(&self, future: F) -> F::Output {
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
async fn register_worker(pool: &PgPool, worker: &str) {
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
struct JobRow<'a> {
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
    fn new(queue: &'a str, id: &'a str, status: &'a str) -> Self {
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

    fn attempts(mut self, attempts: i32) -> Self {
        self.attempts = attempts;
        self
    }

    fn max_attempts(mut self, max_attempts: i32) -> Self {
        self.max_attempts = max_attempts;
        self
    }

    /// Offset the row's `run_at` from now, in seconds; negative is already due.
    fn run_at_offset_seconds(mut self, seconds: f64) -> Self {
        self.run_at_offset_seconds = seconds;
        self
    }

    fn metadata(mut self, metadata: Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    fn locked_by(mut self, worker: &'a str) -> Self {
        self.locked_by = Some(worker);
        self
    }
}

/// Insert one job row with explicit scheduling state.
async fn insert_job(pool: &PgPool, row: JobRow<'_>) {
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
struct FetchedTask {
    task_id: String,
    attempt: usize,
    max_attempts: i32,
}

/// Poll the queue through the real Apalis backend for one due task.
///
/// The stream yields one registration heartbeat (`Ok(None)`) before the first
/// fetch. A window that expires means no further row became due, which is how
/// the negative canaries express "not fetched".
fn fetch_one_task(
    environment: &CanaryEnvironment,
    queue: &str,
    worker: &str,
) -> Option<FetchedTask> {
    environment.run(async {
        let storage =
            PostgresStorage::<Value>::new_with_config(&environment.pool, &Config::new(queue));
        let stream = Backend::poll(storage, &WorkerContext::new::<Value>(worker));
        futures::pin_mut!(stream);

        loop {
            match tokio::time::timeout(FETCH_WINDOW, stream.next()).await {
                Ok(Some(Ok(Some(task)))) => return Some(describe(&task)),
                Ok(Some(Ok(None))) => {}
                Ok(Some(Err(error))) => panic!("fetch failed: {error}"),
                Ok(None) | Err(_) => return None,
            }
        }
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
struct RowState {
    status: String,
    attempts: i32,
    lock_by: Option<String>,
}

fn read_state(runtime: &Runtime, pool: &PgPool, id: &str) -> RowState {
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

#[test]
fn scheduling_columns_exist() {
    let environment = CanaryEnvironment::shared();
    let columns: Vec<String> = environment
        .run(async {
            sqlx::query_scalar(
                "SELECT column_name::text
                 FROM information_schema.columns
                 WHERE table_schema = 'apalis' AND table_name = 'jobs'",
            )
            .fetch_all(&environment.pool)
            .await
        })
        .expect("read apalis.jobs columns");

    for required in ["run_at", "max_attempts", "metadata", "attempts", "status"] {
        assert!(
            columns.iter().any(|column| column == required),
            "apalis.jobs must keep its {required} column; found {columns:?}"
        );
    }
}

#[test]
fn failed_row_with_spare_attempts_is_fetched_when_due() {
    let environment = CanaryEnvironment::shared();
    let pool = environment.pool.clone();
    let queue = "canary.due";
    let worker = "canary-due-worker";
    let id = apalis_job_id();

    environment.run(async {
        register_worker(&pool, worker).await;
        insert_job(&pool, JobRow::new(queue, &id, "Failed")).await;
    });

    let fetched =
        fetch_one_task(environment, queue, worker).expect("a due Failed row must be fetched");
    assert_eq!(fetched.task_id, id);

    // Proving the claim landed keeps this positive case from passing on a
    // fetch that never touched the row. The fetcher stops at the queue step,
    // so the row is `Queued` rather than the `Running` the worker's own lock
    // step would set.
    let state = read_state(&environment.runtime, &pool, &id);
    assert_eq!(
        state.status, "Queued",
        "fetch queues the row for the worker"
    );
    assert_eq!(state.lock_by.as_deref(), Some(worker));
}

/// A row the fetcher must overlook, with the reason it is ineligible.
struct IneligibleRow {
    queue: &'static str,
    worker: &'static str,
    status: &'static str,
    attempts: i32,
    run_at_offset_seconds: f64,
    expectation: &'static str,
}

#[rstest::rstest]
#[case::run_at_in_the_future(IneligibleRow {
    queue: "canary.future",
    worker: "canary-future-worker",
    status: "Failed",
    attempts: 1,
    run_at_offset_seconds: 3_600.0,
    expectation: "a row scheduled in the future must not be fetched",
})]
#[case::killed_row(IneligibleRow {
    queue: "canary.killed",
    worker: "canary-killed-worker",
    status: "Killed",
    attempts: 1,
    run_at_offset_seconds: -1.0,
    expectation: "a Killed row must never be fetched again",
})]
#[case::attempt_cap_reached(IneligibleRow {
    queue: "canary.exhausted",
    worker: "canary-exhausted-worker",
    status: "Failed",
    attempts: 5,
    run_at_offset_seconds: -1.0,
    expectation: "a Failed row at its attempt cap must not be fetched",
})]
fn ineligible_rows_are_not_fetched(#[case] case: IneligibleRow) {
    let environment = CanaryEnvironment::shared();
    let pool = environment.pool.clone();
    let id = apalis_job_id();
    let row = JobRow::new(case.queue, &id, case.status)
        .attempts(case.attempts)
        .run_at_offset_seconds(case.run_at_offset_seconds);

    environment.run(async {
        register_worker(&pool, case.worker).await;
        insert_job(&pool, row).await;
    });

    let fetched = fetch_one_task(environment, case.queue, case.worker);
    assert!(fetched.is_none(), "{}: {fetched:?}", case.expectation);
    assert_eq!(
        read_state(&environment.runtime, &pool, &id),
        RowState {
            status: case.status.to_owned(),
            attempts: case.attempts,
            lock_by: None,
        },
        "{} must stay unclaimed",
        case.status
    );
}

#[test]
fn fetch_preserves_the_persisted_attempt_count() {
    let environment = CanaryEnvironment::shared();
    let pool = environment.pool.clone();
    let queue = "canary.attempts";
    let worker = "canary-attempts-worker";
    let id = apalis_job_id();

    environment.run(async {
        register_worker(&pool, worker).await;
        insert_job(
            &pool,
            JobRow::new(queue, &id, "Failed")
                .attempts(3)
                .max_attempts(10),
        )
        .await;
    });

    let fetched =
        fetch_one_task(environment, queue, worker).expect("the persisted attempt row is due");
    assert_eq!(
        fetched.attempt, 3,
        "the wrapper must see the persisted attempt count, not a reset one"
    );
    assert_eq!(fetched.max_attempts, 10);
}
