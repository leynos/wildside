//! Scenario state and PostgreSQL operations for retry policy BDD tests.

use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use backend::domain::jobs::retry::{
    JobRetryPolicy, JobRetryPolicyConfig, MAX_JOB_FAILURE_MESSAGE_BYTES,
};
use backend::domain::ports::{RouteDeadLetterOutcome, RouteQueue, RouteRetryOutcome};
use backend::outbound::queue::{ApalisPostgresProvider, GenericApalisRouteQueue};
use pg_embedded_setup_unpriv::TemporaryDatabase;
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::runtime::Runtime;

use super::handler::RetryProbeHandler;
use super::metrics::RecordingRetryMetrics;
use crate::support::atexit_cleanup::{ensure_stable_cluster_environment, shared_cluster_handle};
use crate::support::embedded_postgres::provision_template_database;
use crate::support::queue_retry_worker::{
    QueueRetryWorkerConfig, WorkerRunReport, run_queue_retry_worker,
};

pub const QUEUE_NAME: &str = "wildside.retry-policy-bdd";

#[derive(Clone, Copy, Debug)]
pub enum RetryHandlerMode {
    FailRetryableThenSucceed(usize),
    AlwaysRetryable,
    Fatal,
}

#[derive(Clone, Debug)]
pub struct AttemptObservation {
    pub attempt: u32,
    pub started_at_micros: i64,
    pub run_at_micros: i64,
}

#[derive(Clone, Debug)]
pub struct StoredJob {
    pub status: String,
    pub attempts: i32,
    pub max_attempts: i32,
    pub run_at_micros: i64,
    dead_reason: Option<String>,
    pub last_result: Option<String>,
    pub unrelated_metadata: Option<String>,
}

impl StoredJob {
    pub fn is_dead(&self) -> bool {
        self.status == "Killed" || (self.status == "Failed" && self.attempts >= self.max_attempts)
    }

    pub fn dead_reason(&self) -> &str {
        self.dead_reason.as_deref().expect("D15 terminal reason")
    }
}

pub struct QueueRetryPolicyWorld {
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
    pub fn new() -> Self {
        ensure_stable_cluster_environment().expect("stable PostgreSQL environment");
        let runtime = Runtime::new().expect("create BDD Tokio runtime");
        let cluster = shared_cluster_handle().expect("start embedded PostgreSQL");
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

    pub fn set_policy(&self, attempts: u32) {
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

    pub fn set_handler_mode(&self, mode: RetryHandlerMode) {
        *self.handler_mode.lock().expect("handler mode mutex") = mode;
    }

    pub fn enqueue_job(&self) {
        let probe_id = self.probe_id.clone();
        let payload = json!({"v": "v1", "probeId": probe_id});
        let queue = self.queue.lock().expect("route queue mutex").clone();
        self.runtime
            .block_on(queue.enqueue(&payload))
            .expect("enqueue retry policy probe job");
    }

    pub fn insert_preexisting_job(&self, attempts: i32) {
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

    pub fn set_unrelated_metadata(&self) {
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

    pub fn run_worker(&self) {
        let policy = self.policy.lock().expect("retry policy mutex").clone();
        let mode = *self.handler_mode.lock().expect("handler mode mutex");
        let probe_handler = RetryProbeHandler {
            pool: self.pool.clone(),
            probe_id: self.probe_id.clone(),
            observations: Arc::clone(&self.observations),
            mode,
        };
        let handler = move |_job, attempt| {
            let probe_handler = probe_handler.clone();
            async move { probe_handler.run(attempt).await }
        };
        let config = QueueRetryWorkerConfig {
            pool: self.pool.clone(),
            queue_name: QUEUE_NAME.to_owned(),
            probe_id: self.probe_id.clone(),
            policy,
            metrics: Arc::new(self.metrics.clone()),
            stop_after_poll_intervals: None,
        };
        let report = self
            .runtime
            .block_on(run_queue_retry_worker(config, handler))
            .expect("bounded retry worker run");
        *self.worker_report.lock().expect("worker report mutex") = Some(report);
    }

    pub fn observations(&self) -> Vec<AttemptObservation> {
        self.observations
            .lock()
            .expect("attempt observations mutex")
            .clone()
    }

    pub fn observed_attempts(&self) -> Vec<u32> {
        self.observations()
            .into_iter()
            .map(|observation| observation.attempt)
            .collect()
    }

    pub fn worker_report(&self) -> WorkerRunReport {
        self.worker_report
            .lock()
            .expect("worker report mutex")
            .expect("worker report should be available")
    }

    pub fn job_row(&self) -> StoredJob {
        let probe_id = self.probe_id.clone();
        self.runtime.block_on(async move {
            let row: (
                String,
                i32,
                i32,
                i64,
                Option<String>,
                Option<String>,
                Option<String>,
            ) = sqlx::query_as(
                "SELECT status, attempts, max_attempts,
                    (extract(epoch FROM run_at) * 1000000)::bigint,
                    metadata #>> '{wildside.retry.dead_reason}',
                    last_result #>> '{Err}',
                    metadata #>> '{probe.keep}'
                 FROM apalis.jobs
                 WHERE job_type = $1
                   AND convert_from(job, 'UTF8')::jsonb->>'probeId' = $2",
            )
            .bind(QUEUE_NAME)
            .bind(probe_id)
            .fetch_one(&self.pool)
            .await
            .expect("read retry probe row");
            StoredJob {
                status: row.0,
                attempts: row.1,
                max_attempts: row.2,
                run_at_micros: row.3,
                dead_reason: row.4,
                last_result: row.5,
                unrelated_metadata: row.6,
            }
        })
    }

    pub fn assert_acknowledged_state(&self) {
        let row = self.job_row();
        let observation = self
            .observations()
            .last()
            .cloned()
            .expect("the handler must observe the acknowledged attempt");
        assert_eq!(row.attempts, observation.attempt as i32);
        assert_eq!(row.run_at_micros, observation.run_at_micros);

        let expected_max_attempts = if row.is_dead() {
            row.attempts
        } else {
            self.policy
                .lock()
                .expect("retry policy mutex")
                .max_attempts()
                .get() as i32
        };
        assert_eq!(row.max_attempts, expected_max_attempts);
        if row.status != "Done" {
            assert!(
                row.last_result
                    .as_ref()
                    .is_some_and(|result| result.len() <= MAX_JOB_FAILURE_MESSAGE_BYTES),
                "Apalis ack must retain the bounded failure message"
            );
        }
    }

    pub fn assert_metrics(&self, retries: usize, exhausted: usize, fatal: usize) {
        let retry_outcomes = self
            .metrics
            .retry_outcomes
            .lock()
            .expect("retry metrics mutex");
        assert_eq!(retry_outcomes.len(), retries);
        assert!(
            retry_outcomes
                .iter()
                .all(|outcome| *outcome == RouteRetryOutcome::Scheduled)
        );

        let dead_letter_outcomes = self
            .metrics
            .dead_letter_outcomes
            .lock()
            .expect("dead-letter metrics mutex");
        assert_eq!(
            dead_letter_outcomes
                .iter()
                .filter(|outcome| **outcome == RouteDeadLetterOutcome::Exhausted)
                .count(),
            exhausted
        );
        assert_eq!(
            dead_letter_outcomes
                .iter()
                .filter(|outcome| **outcome == RouteDeadLetterOutcome::Fatal)
                .count(),
            fatal
        );
        assert_eq!(dead_letter_outcomes.len(), exhausted + fatal);
    }
}

fn make_policy(attempts: u32) -> JobRetryPolicy {
    JobRetryPolicy::new(JobRetryPolicyConfig {
        max_attempts: NonZeroU32::new(attempts).expect("positive attempts"),
        base_delay: Duration::from_millis(10),
        multiplier: 2.0,
        max_delay: Duration::from_millis(40),
        jitter_fraction: 0.0,
    })
    .expect("valid millisecond retry policy")
}
