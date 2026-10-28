//! Capped Apalis worker harness for queue retry BDD scenarios.

use std::future::Future;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use apalis_core::backend::poll_strategy::{IntervalStrategy, StrategyBuilder};
use apalis_core::worker::builder::WorkerBuilder;
use apalis_postgres::{Config, PgPool, PostgresStorage};
use backend::domain::jobs::retry::JobRetryPolicy;
use backend::domain::ports::RouteQueueMetrics;
use backend::outbound::queue::{
    PostgresRetryScheduler, RetryingJobService, RetryingJobServiceConfig,
};
use serde_json::Value;

const POLL_INTERVAL: Duration = Duration::from_millis(20);
const MAX_POLL_ITERATIONS: usize = 250;
const QUIET_POLL_ITERATIONS: usize = 5;

/// Outcome captured while the worker remains active after a terminal row state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkerRunReport {
    pub reached_terminal: bool,
    pub runs_at_terminal: usize,
    pub runs_after_quiet: usize,
    pub quiet_period_elapsed: bool,
    pub poll_intervals_elapsed: usize,
}

/// Inputs for the bounded PostgreSQL worker used by retry BDD scenarios.
pub struct QueueRetryWorkerConfig {
    pub pool: PgPool,
    pub queue_name: String,
    pub probe_id: String,
    pub policy: JobRetryPolicy,
    pub metrics: Arc<dyn RouteQueueMetrics>,
    pub stop_after_poll_intervals: Option<usize>,
}

impl QueueRetryWorkerConfig {
    fn validate_poll_limit(&self) -> Result<(), String> {
        if self
            .stop_after_poll_intervals
            .is_some_and(|polls| polls == 0 || polls > MAX_POLL_ITERATIONS)
        {
            return Err(format!(
                "bounded poll count must be between 1 and {MAX_POLL_ITERATIONS}"
            ));
        }
        Ok(())
    }
}

/// Observe one test worker without coupling its polling policy to its wiring.
struct WorkerObservation {
    pool: PgPool,
    queue_name: String,
    probe_id: String,
    runs: Arc<AtomicUsize>,
    stop_after_poll_intervals: Option<usize>,
}

impl WorkerObservation {
    async fn observe(&self) -> WorkerRunReport {
        let mut report = WorkerRunReport {
            reached_terminal: false,
            runs_at_terminal: 0,
            runs_after_quiet: 0,
            quiet_period_elapsed: false,
            poll_intervals_elapsed: 0,
        };
        if let Some(intervals) = self.stop_after_poll_intervals {
            wait_intervals(intervals).await;
            report.poll_intervals_elapsed = intervals;
        } else if let Some(runs_at_terminal) = self.wait_for_terminal().await {
            report.reached_terminal = true;
            report.runs_at_terminal = runs_at_terminal;
            wait_intervals(QUIET_POLL_ITERATIONS).await;
            report.quiet_period_elapsed = true;
        }
        report.runs_after_quiet = self.runs.load(Ordering::SeqCst);
        report
    }

    async fn wait_for_terminal(&self) -> Option<usize> {
        for _ in 0..MAX_POLL_ITERATIONS {
            if is_terminal(&self.pool, &self.queue_name, &self.probe_id).await {
                return Some(self.runs.load(Ordering::SeqCst));
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
        None
    }
}

async fn wait_intervals(intervals: usize) {
    for _ in 0..intervals {
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// Build and run the worker wiring that 5.3.1 can promote, with hard test caps.
pub async fn run_queue_retry_worker<F, Fut>(
    config: QueueRetryWorkerConfig,
    handler: F,
) -> Result<WorkerRunReport, String>
where
    F: Fn(Value, NonZeroU32) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Result<(), backend::domain::jobs::retry::JobFailure>> + Send + 'static,
{
    config.validate_poll_limit()?;
    let QueueRetryWorkerConfig {
        pool,
        queue_name,
        probe_id,
        policy,
        metrics,
        stop_after_poll_intervals,
    } = config;
    let runs = Arc::new(AtomicUsize::new(0));
    let counted_handler = {
        let runs = Arc::clone(&runs);
        move |job, attempt| {
            runs.fetch_add(1, Ordering::SeqCst);
            handler(job, attempt)
        }
    };
    let service_config = RetryingJobServiceConfig::new(
        queue_name.clone(),
        policy,
        PostgresRetryScheduler::new(pool.clone()),
        metrics,
    );
    let service = RetryingJobService::new(service_config, counted_handler);
    let config = Config::new(&queue_name).with_poll_interval(
        StrategyBuilder::new()
            .apply(IntervalStrategy::new(POLL_INTERVAL))
            .build(),
    );
    let storage = PostgresStorage::<Value>::new_with_config(&pool, &config);
    let worker = WorkerBuilder::new("queue-retry-policy-bdd")
        .backend(storage)
        .build(service);
    let result = Arc::new(std::sync::Mutex::new(None));
    let worker_result = Arc::clone(&result);
    let observation = WorkerObservation {
        pool,
        queue_name,
        probe_id,
        runs,
        stop_after_poll_intervals,
    };

    worker
        .run_until_ctx(move |ctx| async move {
            let report = observation.observe().await;
            if let Ok(mut slot) = worker_result.lock() {
                *slot = Some(report);
            }
            ctx.stop()?;
            Ok(())
        })
        .await
        .map_err(|error| format!("retry worker failed: {error}"))?;

    let report = result
        .lock()
        .map_err(|error| format!("worker report lock poisoned: {error}"))?
        .take()
        .ok_or_else(|| "worker stopped without a bounded run report".to_owned())?;
    Ok(report)
}

async fn is_terminal(pool: &PgPool, queue_name: &str, probe_id: &str) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1 FROM apalis.jobs
            WHERE job_type = $1
              AND convert_from(job, 'UTF8')::jsonb->>'probeId' = $2
              AND (
                status = 'Done'
                OR status = 'Killed'
                OR (status = 'Failed' AND (max_attempts IS NULL OR attempts >= max_attempts))
              )
        )",
    )
    .bind(queue_name)
    .bind(probe_id)
    .fetch_one(pool)
    .await
    .unwrap_or(false)
}
