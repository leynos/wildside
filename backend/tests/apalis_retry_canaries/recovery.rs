//! Canaries for the two windows orphan recovery opens.
//!
//! Apalis's orphan recovery returns a stale `Running` row to `Pending` and
//! increments its attempt count, while the fetch predicate accepts a `Pending`
//! row without consulting `max_attempts`. A job retired between its terminal
//! write and its acknowledgement can therefore be handed out again. These
//! canaries pin the wrapper's refusal on the delivery side, and the extent of
//! what a stale acknowledgement can reach on the persistence side.

use std::error::Error as StdError;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use apalis_core::error::BoxDynError;
use apalis_core::layers::Service;
use apalis_core::worker::ext::ack::Acknowledge;
use apalis_postgres::PgAck;
use backend::domain::jobs::retry::{JobRetryPolicy, JobRetryPolicyConfig};
use backend::domain::ports::{NoOpRouteQueueMetrics, RouteQueueMetrics};
use backend::outbound::queue::{
    PostgresRetryScheduler, RetryPersistence, RetryingJobService, RetryingJobServiceConfig,
};
use serde::Deserialize;
use serde_json::Value;

use super::fixtures::{
    CanaryEnvironment, JobRow, fetch_task, insert_job, locked_task, read_state, register_worker,
};
use crate::support::apalis_job_id::apalis_job_id;

/// Typed `v1` envelope the production wrapper decodes before invoking the
/// handler.
#[derive(Clone, Debug, Deserialize)]
struct ProbeJob {
    #[serde(rename = "probeId")]
    #[expect(dead_code, reason = "decoding is the behaviour under test")]
    probe_id: String,
}

/// The attempt cap a terminal write leaves on a retired row.
const RETIRED_CAP: i32 = 5;

/// The attempt count orphan recovery leaves after re-queueing a retired row.
///
/// Recovery increments `attempts`, and the retry policy had already removed
/// every attempt the row was permitted, so the count lands one past the cap.
const RECOVERED_ATTEMPTS: i32 = 6;

/// The queue holding a row orphan recovery returned to the fetcher.
const RECOVERED_QUEUE: &str = "canary.recovery-retired";

/// The worker that fetches the recovered row under the same name that ran the
/// retirement, which is the case the guard exists for.
const RECOVERED_WORKER: &str = "canary-recovery-worker";

fn policy() -> JobRetryPolicy {
    JobRetryPolicy::new(JobRetryPolicyConfig {
        max_attempts: NonZeroU32::new(3).expect("positive attempt limit"),
        base_delay: Duration::from_millis(10),
        multiplier: 2.0,
        max_delay: Duration::from_millis(80),
        jitter_fraction: 0.0,
    })
    .expect("valid canary policy")
}

/// Deliver one fetched task through the production wrapper.
///
/// The handler counts its invocations, so a refusal is proved by the count
/// rather than inferred from the returned error.
fn deliver(
    environment: &CanaryEnvironment,
    queue: &str,
    task: apalis_postgres::PgTask<Value>,
    runs: &Arc<AtomicUsize>,
) -> Result<(), BoxDynError> {
    let scheduler: Arc<dyn RetryPersistence> =
        Arc::new(PostgresRetryScheduler::new(environment.pool.clone()));
    let metrics: Arc<dyn RouteQueueMetrics> = Arc::new(NoOpRouteQueueMetrics);
    let config = RetryingJobServiceConfig::new(queue.to_owned(), policy(), scheduler, metrics);
    let counted = Arc::clone(runs);
    let handler = move |_job: ProbeJob, _attempt| {
        counted.fetch_add(1, Ordering::SeqCst);
        async { Ok(()) }
    };
    let mut service = RetryingJobService::<ProbeJob, _>::new(config, handler);
    environment.run(service.call(task))
}

/// Scenario: orphan recovery returned a retired row to the fetcher.
///
/// Invariant: the wrapper refuses the delivery without invoking the handler,
/// so a job already classified as terminal is not run again. Without the guard
/// the handler would run once more, and the row's stored cap would not stop it,
/// because the fetch predicate never reads `max_attempts` for a `Pending` row.
#[test]
fn a_recovered_row_past_its_cap_is_refused_before_the_handler_runs() {
    let environment = CanaryEnvironment::shared();
    let pool = environment.pool.clone();
    let id = apalis_job_id();

    environment.run(async {
        register_worker(&pool, RECOVERED_WORKER).await;
        // This is the row orphan recovery leaves behind: `Pending` and
        // immediately fetchable, with the retired cap still stored and the
        // attempt count incremented past it.
        insert_job(
            &pool,
            JobRow::new(RECOVERED_QUEUE, &id, "Pending")
                .attempts(RECOVERED_ATTEMPTS)
                .max_attempts(RETIRED_CAP),
        )
        .await;
    });

    let task = fetch_task(environment, RECOVERED_QUEUE, RECOVERED_WORKER);
    let runs = Arc::new(AtomicUsize::new(0));

    let result = deliver(environment, RECOVERED_QUEUE, task, &runs);

    assert!(
        result.is_err(),
        "a refused delivery stays failed for the acknowledgement"
    );
    assert_eq!(
        runs.load(Ordering::SeqCst),
        0,
        "a recovered row past its cap must not reach the handler"
    );
    let state = read_state(&environment.runtime, &pool, &id);
    assert_eq!(
        state.attempts, RECOVERED_ATTEMPTS,
        "the refusal must not rewrite the recovered attempt count"
    );
}

/// The worker that holds the row after the stale delivery lost it.
const REPLACEMENT_WORKER: &str = "canary-recovery-replacement";

/// The worker whose acknowledgement arrives after it lost the row.
const STALE_WORKER: &str = "canary-recovery-stale";

/// Scenario: a delivery acknowledges after another worker took the row over.
///
/// Invariant: the acknowledgement matches no row, so the replacement's
/// `Running` state and attempt count survive. The acknowledgement SQL matches on
/// `id` and `lock_by` alone, which is why distinct worker names are what keep a
/// stale delivery away from another instance's row.
#[test]
fn a_stale_acknowledgement_cannot_overwrite_a_replacements_row() {
    let environment = CanaryEnvironment::shared();
    let pool = environment.pool.clone();
    let id = apalis_job_id();
    const REPLACEMENT_ATTEMPTS: i32 = 2;

    environment.run(async {
        register_worker(&pool, REPLACEMENT_WORKER).await;
        insert_job(
            &pool,
            JobRow::new("canary.recovery-ack", &id, "Running")
                .attempts(REPLACEMENT_ATTEMPTS)
                .max_attempts(RETIRED_CAP)
                .run_at_offset_seconds(3_600.0)
                .locked_by(REPLACEMENT_WORKER),
        )
        .await;
    });
    let before = read_state(&environment.runtime, &pool, &id);

    let acknowledged = environment.run(async {
        // The stale delivery is the one the row used to belong to, so it
        // carries that worker's name and the attempt count it inherited.
        let stale_task = locked_task(
            &id,
            STALE_WORKER,
            RETIRED_CAP,
            REPLACEMENT_ATTEMPTS as usize,
        );
        let result: Result<(), BoxDynError> = Err(Box::new(ProbeFailure));
        PgAck::new(pool.clone())
            .ack(&result, &stale_task.parts)
            .await
    });

    assert!(
        acknowledged.is_err(),
        "a stale acknowledgement must match no row and report that"
    );
    assert_eq!(
        read_state(&environment.runtime, &pool, &id),
        before,
        "the replacement keeps its Running status, attempts, and lock owner"
    );
}

/// The failure an acknowledged delivery reports.
#[derive(Debug)]
struct ProbeFailure;

impl std::fmt::Display for ProbeFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("probe failure")
    }
}

impl StdError for ProbeFailure {}
