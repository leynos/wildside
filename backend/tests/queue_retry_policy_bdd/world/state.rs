//! Stored-state inspection and assertions for retry policy scenarios.
//!
//! Split from the world's setup and drive operations so the scenario state
//! module stays within the repository's module-size limit. These helpers read
//! the rows and metrics a scenario produced; they never schedule work.

use backend::domain::jobs::retry::MAX_JOB_FAILURE_MESSAGE_BYTES;
use backend::domain::ports::{RouteDeadLetterOutcome, RouteRetryOutcome};

use super::{AttemptObservation, BASE_DELAY, QUEUE_NAME, QueueRetryPolicyWorld, StoredJob};
use crate::support::queue_retry_worker::WorkerRunReport;

impl QueueRetryPolicyWorld {
    /// Read the most recently stored row for this scenario's queue.
    ///
    /// Used for payloads that deliberately omit the probe id, which the
    /// probe-id based lookup cannot find.
    pub fn latest_job_row(&self) -> StoredJob {
        self.runtime.block_on(async {
            sqlx::query_as(
                "SELECT status, attempts, max_attempts,
                    (extract(epoch FROM run_at) * 1000000)::bigint,
                    metadata #>> '{wildside.retry.dead_reason}',
                    last_result #>> '{Err}',
                    metadata #>> '{probe.keep}'
                 FROM apalis.jobs
                 WHERE job_type = $1
                 ORDER BY id DESC
                 LIMIT 1",
            )
            .bind(QUEUE_NAME)
            .fetch_one(&self.pool)
            .await
            .map(
                |(
                    status,
                    attempts,
                    max_attempts,
                    run_at_micros,
                    dead_reason,
                    last_result,
                    unrelated_metadata,
                ): (
                    String,
                    i32,
                    i32,
                    i64,
                    Option<String>,
                    Option<String>,
                    Option<String>,
                )| StoredJob {
                    status,
                    attempts,
                    max_attempts,
                    run_at_micros,
                    dead_reason,
                    last_result,
                    unrelated_metadata,
                },
            )
            .expect("read latest retry probe row")
        })
    }

    pub fn probe_id(&self) -> &str {
        &self.probe_id
    }

    /// Configured backoff for the first retry, in microseconds.
    ///
    /// The scheduler stores `now() + delay`, so the gap between consecutive
    /// `run_at` values must be at least this large. A guarded write that never
    /// fired would leave `run_at` unchanged, which this catches.
    pub fn base_delay_micros(&self) -> i64 {
        i64::try_from(BASE_DELAY.as_micros()).expect("base delay fits i64")
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
