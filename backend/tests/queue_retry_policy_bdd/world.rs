//! Scenario state and PostgreSQL operations for retry policy BDD tests.

use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use backend::domain::jobs::retry::{JobRetryPolicy, JobRetryPolicyConfig};
use backend::domain::ports::RouteQueue;
use backend::outbound::queue::{ApalisPostgresProvider, GenericApalisRouteQueue};
use pg_embedded_setup_unpriv::TemporaryDatabase;
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::runtime::Runtime;

use super::handler::RetryProbeHandler;
use super::metrics::RecordingRetryMetrics;
use crate::support::atexit_cleanup::{bootstrap_shared_cluster, ensure_stable_cluster_environment};
use crate::support::embedded_postgres::provision_template_database;
use crate::support::queue_retry_worker::{
    DEFAULT_WORKER_DEADLINE, QueueRetryProbeJob, QueueRetryWorkerConfig, WorkerRunReport,
    run_queue_retry_worker,
};

mod state;

pub(crate) use state::assert_deadlines_advance;

pub(crate) const QUEUE_NAME: &str = "wildside.retry-policy-bdd";

/// First retry delay used by every scenario policy.
const BASE_DELAY: Duration = Duration::from_millis(10);

#[derive(Clone, Copy, Debug)]
pub(crate) enum RetryHandlerMode {
    FailRetryableThenSucceed(usize),
    AlwaysRetryable,
    Fatal,
    /// Fail retryably only after outliving any sensible harness deadline.
    NeverCompletes,
}

#[derive(Clone, Debug)]
pub(crate) struct AttemptObservation {
    pub attempt: u32,
    pub started_at_micros: i64,
    pub run_at_micros: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct StoredJob {
    pub status: String,
    pub attempts: i32,
    pub max_attempts: i32,
    pub run_at_micros: i64,
    dead_reason: Option<String>,
    pub last_result: Option<String>,
    pub unrelated_metadata: Option<String>,
}

impl StoredJob {
    pub(crate) fn is_dead(&self) -> bool {
        self.status == "Killed" || (self.status == "Failed" && self.attempts >= self.max_attempts)
    }

    pub(crate) fn dead_reason(&self) -> &str {
        self.dead_reason.as_deref().expect("D15 terminal reason")
    }
}

pub(crate) struct QueueRetryPolicyWorld {
    runtime: Runtime,
    pool: PgPool,
    _database: TemporaryDatabase,
    policy: Mutex<JobRetryPolicy>,
    queue: Mutex<GenericApalisRouteQueue<Value, ApalisPostgresProvider>>,
    handler_mode: Mutex<RetryHandlerMode>,
    probe_id: String,
    observations: Arc<Mutex<Vec<AttemptObservation>>>,
    worker_report: Mutex<Option<WorkerRunReport>>,
    metrics: RecordingRetryMetrics,
}

impl Default for QueueRetryPolicyWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl QueueRetryPolicyWorld {
    pub(crate) fn new() -> Self {
        ensure_stable_cluster_environment().expect("stable PostgreSQL environment");
        let runtime = Runtime::new().expect("create BDD Tokio runtime");
        let cluster = bootstrap_shared_cluster().expect("start embedded PostgreSQL");
        let database = provision_template_database(cluster).expect("provision BDD database");
        let database_url = database.url().to_string();
        let pool = runtime
            .block_on(PgPool::connect(&database_url))
            .expect("connect Apalis pool");
        let policy = make_policy(3);
        let provider = runtime
            .block_on(ApalisPostgresProvider::new(
                pool.clone(),
                QUEUE_NAME,
                policy.clone(),
            ))
            .expect("setup Apalis retry queue");
        let metrics = RecordingRetryMetrics::default();
        let queue = GenericApalisRouteQueue::new(provider, Arc::new(metrics.clone()));
        Self {
            runtime,
            pool,
            _database: database,
            policy: Mutex::new(policy),
            queue: Mutex::new(queue),
            handler_mode: Mutex::new(RetryHandlerMode::AlwaysRetryable),
            probe_id: uuid::Uuid::new_v4().to_string(),
            observations: Arc::new(Mutex::new(Vec::new())),
            worker_report: Mutex::new(None),
            metrics,
        }
    }

    pub(crate) fn set_policy(&self, attempts: u32) {
        let policy = make_policy(attempts);
        let provider = self
            .runtime
            .block_on(ApalisPostgresProvider::new(
                self.pool.clone(),
                QUEUE_NAME,
                policy.clone(),
            ))
            .expect("create queue with scenario retry policy");
        *self.queue.lock().expect("route queue mutex") =
            GenericApalisRouteQueue::new(provider, Arc::new(self.metrics.clone()));
        *self.policy.lock().expect("retry policy mutex") = policy;
    }

    pub(crate) fn set_handler_mode(&self, mode: RetryHandlerMode) {
        *self.handler_mode.lock().expect("handler mode mutex") = mode;
    }

    pub(crate) fn enqueue_job(&self) {
        let probe_id = self.probe_id.clone();
        let payload = json!({"v": "v1", "probeId": probe_id});
        let queue = self.queue.lock().expect("route queue mutex").clone();
        self.runtime
            .block_on(queue.enqueue(&payload))
            .expect("enqueue retry policy probe job");
    }

    pub(crate) fn insert_preexisting_job(&self, attempts: i32) {
        let probe_id = self.probe_id.clone();
        let payload = json!({"v": "v1", "probeId": probe_id});
        let pool = self.pool.clone();
        self.runtime.block_on(async move {
            let mut storage = apalis_postgres::PostgresStorage::<Value>::new_with_config(
                &pool,
                &apalis_postgres::Config::new(QUEUE_NAME),
            );
            let task = apalis_core::task::Task::builder(payload)
                .with_ctx(apalis_postgres::PgContext::new().with_max_attempts(attempts))
                .build();
            use apalis_core::backend::TaskSink;
            storage
                .push_task(task)
                .await
                .expect("insert pre-existing attempt budget");
        });
    }

    pub(crate) fn set_unrelated_metadata(&self) {
        let probe_id = self.probe_id.clone();
        self.runtime
            .block_on(
                sqlx::query(
                    "UPDATE apalis.jobs
                 SET metadata = jsonb_set(
                     COALESCE(metadata, '{}'::jsonb),
                     '{probe.keep}',
                     to_jsonb('retain-me'::text),
                     true
                 )
                 WHERE job_type = $1
                   AND convert_from(job, 'UTF8')::jsonb->>'probeId' = $2",
                )
                .bind(QUEUE_NAME)
                .bind(probe_id)
                .execute(&self.pool),
            )
            .expect("set unrelated metadata marker");
    }

    pub(crate) fn run_worker(&self) {
        self.run_worker_with_deadline(DEFAULT_WORKER_DEADLINE)
            .expect("bounded retry worker run");
    }

    /// Run the worker with a fixed observation window.
    ///
    /// Used when the stored payload cannot be located by probe id, so the
    /// harness cannot wait for a terminal state and must bound the run by poll
    /// count instead.
    pub(crate) fn run_worker_for_polls(&self, polls: usize) {
        self.run_worker_config(DEFAULT_WORKER_DEADLINE, Some(polls))
            .expect("bounded retry worker run");
    }

    pub(crate) fn run_worker_with_deadline(&self, deadline: Duration) -> Result<(), String> {
        self.run_worker_config(deadline, None)
    }

    fn run_worker_config(
        &self,
        deadline: Duration,
        stop_after_poll_intervals: Option<usize>,
    ) -> Result<(), String> {
        let policy = self.policy.lock().expect("retry policy mutex").clone();
        let mode = *self.handler_mode.lock().expect("handler mode mutex");
        let probe_handler = RetryProbeHandler {
            pool: self.pool.clone(),
            probe_id: self.probe_id.clone(),
            observations: Arc::clone(&self.observations),
            mode,
        };
        let handler = move |_job: QueueRetryProbeJob, attempt| {
            let probe_handler = probe_handler.clone();
            async move { probe_handler.run(attempt).await }
        };
        let config = QueueRetryWorkerConfig {
            pool: self.pool.clone(),
            queue_name: QUEUE_NAME.to_owned(),
            probe_id: self.probe_id.clone(),
            policy,
            metrics: Arc::new(self.metrics.clone()),
            stop_after_poll_intervals,
            deadline,
        };
        let report = self
            .runtime
            .block_on(run_queue_retry_worker(config, handler))?;
        *self.worker_report.lock().expect("worker report mutex") = Some(report);
        Ok(())
    }

    /// Store a job whose body may not decode against the queue's job type.
    ///
    /// The payload is pushed straight into `apalis.jobs` so the wrapper, not
    /// the enqueue adapter, is the component that rejects it.
    pub(crate) fn insert_raw_job(&self, payload: Value) {
        let pool = self.pool.clone();
        let attempts = self
            .policy
            .lock()
            .expect("retry policy mutex")
            .max_attempts()
            .get();
        let max_attempts = i32::try_from(attempts).expect("policy attempts fit i32");
        self.runtime.block_on(async move {
            let mut storage = apalis_postgres::PostgresStorage::<Value>::new_with_config(
                &pool,
                &apalis_postgres::Config::new(QUEUE_NAME),
            );
            let task = apalis_core::task::Task::builder(payload)
                .with_ctx(apalis_postgres::PgContext::new().with_max_attempts(max_attempts))
                .build();
            use apalis_core::backend::TaskSink;
            storage
                .push_task(task)
                .await
                .expect("insert raw retry probe job");
        });
    }
}

fn make_policy(attempts: u32) -> JobRetryPolicy {
    JobRetryPolicy::new(JobRetryPolicyConfig {
        max_attempts: NonZeroU32::new(attempts).expect("positive attempts"),
        base_delay: BASE_DELAY,
        multiplier: 2.0,
        max_delay: Duration::from_millis(40),
        jitter_fraction: 0.0,
    })
    .expect("valid millisecond retry policy")
}
