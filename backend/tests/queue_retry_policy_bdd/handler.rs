//! Retry probe handler used by the queue policy BDD world.

use std::sync::{Arc, Mutex};

use backend::domain::jobs::retry::{JobErrorKind, JobFailure};
use sqlx::PgPool;

use super::world::{AttemptObservation, QUEUE_NAME, RetryHandlerMode};

#[derive(Clone)]
pub(super) struct RetryProbeHandler {
    pub(super) pool: PgPool,
    pub(super) probe_id: String,
    pub(super) observations: Arc<Mutex<Vec<AttemptObservation>>>,
    pub(super) mode: RetryHandlerMode,
}

impl RetryProbeHandler {
    pub(super) async fn run(&self, attempt: std::num::NonZeroU32) -> Result<(), JobFailure> {
        let (started_at_micros, run_at_micros): (i64, i64) = sqlx::query_as(
            "SELECT
                (extract(epoch FROM clock_timestamp()) * 1000000)::bigint,
                (extract(epoch FROM run_at) * 1000000)::bigint
             FROM apalis.jobs
             WHERE job_type = $1
               AND convert_from(job, 'UTF8')::jsonb->>'probeId' = $2",
        )
        .bind(QUEUE_NAME)
        .bind(&self.probe_id)
        .fetch_one(&self.pool)
        .await
        .expect("read current retry scheduling timestamps");
        self.observations
            .lock()
            .expect("attempt observations mutex")
            .push(AttemptObservation {
                attempt: attempt.get(),
                started_at_micros,
                run_at_micros,
            });

        let invocation = self
            .observations
            .lock()
            .expect("attempt observations mutex")
            .len();
        match self.mode {
            RetryHandlerMode::FailRetryableThenSucceed(failures) if invocation > failures => Ok(()),
            RetryHandlerMode::Fatal => Err(JobFailure::new(
                JobErrorKind::Fatal,
                "permanent probe failure",
            )),
            RetryHandlerMode::FailRetryableThenSucceed(_) | RetryHandlerMode::AlwaysRetryable => {
                Err(JobFailure::new(
                    JobErrorKind::Retryable,
                    "temporary probe failure",
                ))
            }
        }
    }
}
