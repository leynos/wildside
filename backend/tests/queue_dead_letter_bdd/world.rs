//! Scenario state and real PostgreSQL operations for dead-letter behaviour.

mod assertions;
mod dead_job_fixtures;
mod plan;
mod rows;
use self::rows::JobSeed;
use self::rows::JobSnapshot;

use std::num::NonZeroU32;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use apalis_postgres::PgPool;
use backend::domain::jobs::retry::{
    JobErrorKind, JobFailure, JobRetryPolicy, JobRetryPolicyConfig,
};
use backend::domain::ports::{
    DeadJob, DeadJobId, DeadLetterError, DeadLetterQueue, NoOpRouteQueueMetrics, RouteQueue,
};
use backend::outbound::queue::{
    ApalisPostgresProvider, GenericApalisRouteQueue, PostgresDeadLetterQueue, RequeueBudgets,
};
use pg_embedded_setup_unpriv::TemporaryDatabase;
use serde_json::{Value, json};
use tokio::runtime::Runtime;

use crate::support::atexit_cleanup::{ensure_stable_cluster_environment, shared_cluster_handle};
use crate::support::embedded_postgres::provision_template_database;
use crate::support::queue_retry_worker::{
    QueueRetryWorkerConfig, WorkerRunReport, run_queue_retry_worker,
};

const QUEUE_NAME: &str = "wildside.dead-letter-bdd";
const WORKER_NAME: &str = "queue-retry-policy-bdd";
const LEGACY_ROW_POLL_INTERVALS: usize = 10;

#[derive(Clone, Copy)]
enum HandlerMode {
    Succeed,
    RetryableFailure,
    FatalFailure,
}

#[derive(Clone, Copy)]
enum DeadJobOperation {
    Requeue,
    Discard,
}

/// A scenario owns a disposable Apalis database and exercises the public port.
pub struct QueueDeadLetterWorld {
    runtime: Runtime,
    pool: PgPool,
    _database: TemporaryDatabase,
    policy: JobRetryPolicy,
    queue: GenericApalisRouteQueue<Value, ApalisPostgresProvider>,
    dead_letters: PostgresDeadLetterQueue,
    probe_id: String,
    handler_mode: Mutex<HandlerMode>,
    handler_calls: Mutex<usize>,
    current_job_id: Mutex<Option<DeadJobId>>,
    poll_control_job_id: Mutex<Option<DeadJobId>>,
    newest_job_id: Mutex<Option<DeadJobId>>,
    listing: Mutex<Vec<DeadJob>>,
    operation_result: Mutex<Option<Result<(), DeadLetterError>>>,
    state_before_operation: Mutex<Option<JobSnapshot>>,
}

impl Default for QueueDeadLetterWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl QueueDeadLetterWorld {
    pub fn new() -> Self {
        ensure_stable_cluster_environment().expect("stable embedded PostgreSQL environment");
        let runtime = Runtime::new().expect("create dead-letter Tokio runtime");
        let cluster = shared_cluster_handle().expect("start embedded PostgreSQL");
        let database = provision_template_database(cluster).expect("provision test database");
        let pool = runtime
            .block_on(PgPool::connect(database.url()))
            .expect("connect test PostgreSQL pool");
        let policy = retry_policy();
        let provider = runtime
            .block_on(ApalisPostgresProvider::new(
                pool.clone(),
                QUEUE_NAME,
                policy.clone(),
            ))
            .expect("setup Apalis queue");
        let queue = GenericApalisRouteQueue::new(provider, Arc::new(NoOpRouteQueueMetrics));
        let dead_letters = PostgresDeadLetterQueue::new(
            pool.clone(),
            RequeueBudgets::from_policies([(QUEUE_NAME, policy.clone())]),
        );

        Self {
            runtime,
            pool,
            _database: database,
            policy,
            queue,
            dead_letters,
            probe_id: uuid::Uuid::new_v4().to_string(),
            handler_mode: Mutex::new(HandlerMode::RetryableFailure),
            handler_calls: Mutex::new(0),
            current_job_id: Mutex::new(None),
            poll_control_job_id: Mutex::new(None),
            newest_job_id: Mutex::new(None),
            listing: Mutex::new(Vec::new()),
            operation_result: Mutex::new(None),
            state_before_operation: Mutex::new(None),
        }
    }

    pub fn exhaust_job(&self) {
        *self.handler_mode.lock().expect("handler mode mutex") = HandlerMode::RetryableFailure;
        self.enqueue_probe_job();
        self.process_queue();
        self.remember_probe_job_id();
    }

    pub fn fix_fault(&self) {
        *self.handler_mode.lock().expect("handler mode mutex") = HandlerMode::Succeed;
    }

    pub fn allow_null_attempt_limit(&self) {
        self.runtime
            .block_on(
                sqlx::query("ALTER TABLE apalis.jobs ALTER COLUMN max_attempts DROP NOT NULL")
                    .execute(&self.pool),
            )
            .expect("allow legacy nullable attempt limits");
    }

    pub fn insert_legacy_failed_row(&self, attempts: i32) {
        let id = self.insert_job(JobSeed {
            job_type: QUEUE_NAME,
            status: "Failed",
            attempts,
            max_attempts: None,
            reason: None,
            last_error: Some("legacy failure"),
            done_age_seconds: 0,
        });
        *self.current_job_id.lock().expect("current job id mutex") = Some(id);
    }

    pub fn insert_pending_poll_control(&self) {
        self.fix_fault();
        let id = self.insert_pending_row();
        *self
            .poll_control_job_id
            .lock()
            .expect("poll control job id mutex") = Some(id);
    }

    pub fn poll_queue_for_bounded_period(&self) {
        let report = self.run_worker(Some(LEGACY_ROW_POLL_INTERVALS));
        assert_eq!(report.poll_intervals_elapsed, LEGACY_ROW_POLL_INTERVALS);
        assert!(self.worker_was_registered());
    }

    fn worker_was_registered(&self) -> bool {
        self.runtime
            .block_on(
                sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM apalis.workers WHERE id = $1)")
                    .bind(WORKER_NAME)
                    .fetch_one(&self.pool),
            )
            .expect("check bounded-poll worker registration")
    }

    pub fn fatal_job(&self) {
        *self.handler_mode.lock().expect("handler mode mutex") = HandlerMode::FatalFailure;
        self.enqueue_probe_job();
        self.process_queue();
        self.remember_probe_job_id();
    }

    pub fn fail_retryably(&self) {
        *self.handler_mode.lock().expect("handler mode mutex") = HandlerMode::RetryableFailure;
    }

    pub fn insert_pending_job(&self) {
        let id = self.insert_pending_row();
        *self.current_job_id.lock().expect("current id mutex") = Some(id);
    }

    fn insert_pending_row(&self) -> DeadJobId {
        self.insert_job(JobSeed {
            job_type: QUEUE_NAME,
            status: "Pending",
            attempts: 0,
            max_attempts: Some(3),
            reason: None,
            last_error: None,
            done_age_seconds: 0,
        })
    }

    pub fn insert_pending_and_running_jobs(&self) {
        self.insert_pending_row();
        self.insert_job(JobSeed {
            job_type: QUEUE_NAME,
            status: "Running",
            attempts: 1,
            max_attempts: Some(3),
            reason: None,
            last_error: None,
            done_age_seconds: 0,
        });
    }

    pub fn list_dead(&self, limit: u32) {
        let result = self.runtime.block_on(self.dead_letters.list_dead(limit));
        *self.listing.lock().expect("listing mutex") = result.expect("list dead jobs");
    }

    pub fn requeue_dead_job(&self) {
        self.run_dead_job_operation(DeadJobOperation::Requeue);
    }

    pub fn requeue_current_job(&self) {
        self.record_current_job_state();
        self.requeue_dead_job();
    }

    pub fn requeue_current_dead_job(&self) {
        self.requeue_current_job();
    }

    pub fn add_requeue_metadata(&self) {
        let metadata = json!({
            "wildside.retry.requeue_count": 4,
            "operator.note": "preserve-me"
        });
        self.runtime
            .block_on(
                sqlx::query(
                    "UPDATE apalis.jobs
                     SET metadata = COALESCE(metadata, '{}'::jsonb) || $2
                     WHERE id = $1",
                )
                .bind(self.current_id().as_str())
                .bind(metadata)
                .execute(&self.pool),
            )
            .expect("add existing requeue metadata");
    }

    pub fn discard_dead_job(&self) {
        self.run_dead_job_operation(DeadJobOperation::Discard);
    }

    pub fn discard_current_job(&self) {
        self.record_current_job_state();
        self.discard_dead_job();
    }

    fn record_current_job_state(&self) {
        let id = self.current_id();
        let before = self.snapshot(&id);
        *self.state_before_operation.lock().expect("snapshot mutex") = Some(before);
    }

    fn run_dead_job_operation(&self, operation: DeadJobOperation) {
        let id = self.current_id();
        let result = self.runtime.block_on(async {
            match operation {
                DeadJobOperation::Requeue => self.dead_letters.requeue(&id).await,
                DeadJobOperation::Discard => self.dead_letters.discard(&id).await,
            }
        });
        *self
            .operation_result
            .lock()
            .expect("operation result mutex") = Some(result);
    }

    pub fn process_queue(&self) {
        let report = self.run_worker(None);
        assert!(report.reached_terminal, "worker reached a terminal state");
    }

    fn run_worker(&self, stop_after_poll_intervals: Option<usize>) -> WorkerRunReport {
        let mode = *self.handler_mode.lock().expect("handler mode mutex");
        let calls = Arc::new(AtomicUsize::new(0));
        let handler_calls = Arc::clone(&calls);
        let handler = move |_job: Value, _attempt: NonZeroU32| {
            handler_calls.fetch_add(1, Ordering::SeqCst);
            async move {
                match mode {
                    HandlerMode::Succeed => Ok(()),
                    HandlerMode::RetryableFailure => Err(JobFailure::new(
                        JobErrorKind::Retryable,
                        "temporary dead-letter probe failure",
                    )),
                    HandlerMode::FatalFailure => Err(JobFailure::new(
                        JobErrorKind::Fatal,
                        "permanent dead-letter probe failure",
                    )),
                }
            }
        };
        let report = self
            .runtime
            .block_on(run_queue_retry_worker(
                QueueRetryWorkerConfig {
                    pool: self.pool.clone(),
                    queue_name: QUEUE_NAME.to_owned(),
                    probe_id: self.probe_id.clone(),
                    policy: self.policy.clone(),
                    metrics: Arc::new(NoOpRouteQueueMetrics),
                    stop_after_poll_intervals,
                },
                handler,
            ))
            .expect("run bounded retry worker");
        *self.handler_calls.lock().expect("handler calls mutex") = calls.load(Ordering::SeqCst);
        report
    }

    fn enqueue_probe_job(&self) {
        self.runtime
            .block_on(self.queue.enqueue(&json!({
                "v": "v1",
                "probeId": self.probe_id
            })))
            .expect("enqueue retry probe");
    }

    fn take_operation_result(&self) -> Result<(), DeadLetterError> {
        self.operation_result
            .lock()
            .expect("operation result mutex")
            .take()
            .expect("operation result")
    }
}

fn retry_policy() -> JobRetryPolicy {
    JobRetryPolicy::new(JobRetryPolicyConfig {
        max_attempts: NonZeroU32::new(3).expect("positive retry count"),
        base_delay: Duration::from_millis(1),
        multiplier: 1.0,
        max_delay: Duration::from_millis(1),
        jitter_fraction: 0.0,
    })
    .expect("valid BDD retry policy")
}
