//! Row and delivery builders shared by the canaries.
//!
//! These helpers create rows with explicit scheduling state and reduce a row or
//! a fetched task to the fields a canary asserts on. The two fetch helpers
//! answer different questions, as the parent module explains; the negative
//! canaries use the direct claim because an expired window cannot tell an empty
//! poll from one that never completed.

use std::time::Duration;

use apalis_core::backend::Backend;
use apalis_core::task::Task;
use apalis_core::task::attempt::Attempt;
use apalis_core::worker::context::WorkerContext;
use apalis_postgres::{Config, PgContext, PgTask, PostgresStorage};
use futures::StreamExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::runtime::Runtime;

use super::environment::CanaryEnvironment;

/// How long to watch the fetcher before concluding a delivery is not coming.
const FETCH_WINDOW: Duration = Duration::from_millis(1_500);

/// Scheduling state for one canary job row.
///
/// The defaults describe the row the wrapper most often has to reason about:
/// a `Failed` job that is due now, has spent one of its five attempts, and
/// carries no metadata or worker lock. Cases adjust only the field they are
/// about.
pub(crate) struct JobRow<'a> {
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
    pub(crate) fn new(queue: &'a str, id: &'a str, status: &'a str) -> Self {
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

    pub(crate) fn attempts(mut self, attempts: i32) -> Self {
        self.attempts = attempts;
        self
    }

    pub(crate) fn max_attempts(mut self, max_attempts: i32) -> Self {
        self.max_attempts = max_attempts;
        self
    }

    /// Offset the row's `run_at` from now, in seconds; negative is already due.
    pub(crate) fn run_at_offset_seconds(mut self, seconds: f64) -> Self {
        self.run_at_offset_seconds = seconds;
        self
    }

    pub(crate) fn metadata(mut self, metadata: Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub(crate) fn locked_by(mut self, worker: &'a str) -> Self {
        self.locked_by = Some(worker);
        self
    }
}

/// Insert one job row with explicit scheduling state.
pub(crate) async fn insert_job(pool: &PgPool, row: JobRow<'_>) {
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
pub(crate) struct FetchedTask {
    pub(crate) task_id: String,
    pub(crate) attempt: usize,
    pub(crate) max_attempts: i32,
}

/// Poll the queue through the real Apalis backend until one task is delivered.
///
/// The stream yields one registration heartbeat (`Ok(None)`) before the first
/// fetch. A window that expires leaves the positive canary unproven rather
/// than failed, so it panics with that reading instead of reporting "not
/// fetched"; the negative canaries use [`claim_due_rows`], whose result they
/// can interpret.
pub(crate) fn fetch_one_task(
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
pub(crate) fn fetch_task(
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
pub(crate) fn claim_due_rows(environment: &CanaryEnvironment, queue: &str, worker: &str) -> i64 {
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
pub(crate) struct RowState {
    pub(crate) status: String,
    pub(crate) attempts: i32,
    pub(crate) lock_by: Option<String>,
    pub(crate) claim_token: Option<String>,
}

pub(crate) fn read_state(runtime: &Runtime, pool: &PgPool, id: &str) -> RowState {
    runtime.block_on(async {
        let row: (String, i32, Option<String>, Option<String>) = sqlx::query_as(
            "SELECT status, attempts, lock_by, metadata->>'wildside.retry.claim_token'
             FROM apalis.jobs WHERE id = $1",
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("read canary job state");
        RowState {
            status: row.0,
            attempts: row.1,
            lock_by: row.2,
            claim_token: row.3,
        }
    })
}

/// Persisted state a guarded retry-scheduler write could change.
///
/// Every column here is one an owned write names or rewrites, so comparing two
/// readings proves a refused write changed nothing at all.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct GuardedRowState {
    pub(crate) status: String,
    pub(crate) attempts: i32,
    pub(crate) max_attempts: i32,
    pub(crate) lock_by: Option<String>,
    pub(crate) run_at_micros: i64,
    pub(crate) metadata: Option<String>,
    pub(crate) claim_token: Option<String>,
}

/// Read every column a guarded retry-scheduler write could change.
pub(crate) fn read_guarded_row(runtime: &Runtime, pool: &PgPool, id: &str) -> GuardedRowState {
    runtime.block_on(async {
        let (status, attempts, max_attempts, lock_by, run_at_micros, metadata, claim_token) =
            sqlx::query_as(
                "SELECT status, attempts, max_attempts, lock_by,
                        (extract(epoch FROM run_at) * 1000000)::bigint,
                        metadata::text,
                        metadata->>'wildside.retry.claim_token'
                 FROM apalis.jobs WHERE id = $1",
            )
            .bind(id)
            .fetch_one(pool)
            .await
            .expect("read canary guarded row state");
        GuardedRowState {
            status,
            attempts,
            max_attempts,
            lock_by,
            run_at_micros,
            metadata,
            claim_token,
        }
    })
}

/// Identity of one delivery, as the task context carries it.
///
/// The four fields travel together because Apalis reads the worker, the attempt
/// limit, and the attempt from the context and the acknowledgement compares
/// them with the row. Passing them positionally would let a canary pair one
/// delivery's worker with another's attempt, which no worker ever produces.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Delivery<'a> {
    worker: &'a str,
    max_attempts: i32,
    attempt: usize,
    claim_token: Option<&'a str>,
}

impl<'a> Delivery<'a> {
    /// A delivery as the pinned backend leaves it, carrying no claim token.
    pub(crate) fn new(worker: &'a str, max_attempts: i32, attempt: usize) -> Self {
        Self {
            worker,
            max_attempts,
            attempt,
            claim_token: None,
        }
    }

    /// The same delivery carrying the token its claim stamped on the row.
    ///
    /// [`AttemptBoundAck`](backend::outbound::queue::AttemptBoundAck) binds the
    /// claim token of the generation that locked the row, so a delivery built
    /// for it must carry the token its own claim wrote.
    pub(crate) fn claimed(mut self, claim_token: &'a str) -> Self {
        self.claim_token = Some(claim_token);
        self
    }
}

/// A running row reduced to the delivery a worker hands the wrapper.
///
/// The attempt and its limit travel in the task rather than being read from the
/// row, because that is where Apalis takes them from when it acknowledges a
/// delivery: it decides the status from the context it cloned before the
/// handler ran.
pub(crate) fn delivery_task(id: &str, delivery: Delivery<'_>) -> PgTask<Value> {
    let mut meta = serde_json::Map::new();
    if let Some(token) = delivery.claim_token {
        meta.insert(
            "wildside.retry.claim_token".to_owned(),
            Value::String(token.to_owned()),
        );
    }
    Task::builder(json!({ "v": "v1", "probeId": id }))
        .with_ctx(
            PgContext::new()
                .with_max_attempts(delivery.max_attempts)
                .with_lock_by(Some(delivery.worker.to_owned()))
                .with_meta(meta),
        )
        .with_task_id(id.parse().expect("canary task id"))
        .with_attempt(Attempt::new_with_value(delivery.attempt))
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
