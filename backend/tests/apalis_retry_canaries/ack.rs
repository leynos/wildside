//! Acknowledgement canaries for durable retry scheduling.
//!
//! The wrapper relies on `PgAck` leaving the scheduled deadline, the attempt
//! cap, and unrelated metadata alone when it records a delivery. A failure
//! here means an Apalis upgrade changed the contract, not that the wrapper
//! regressed.

use std::error::Error as StdError;

use apalis_core::error::BoxDynError;
use apalis_core::task::Task;
use apalis_core::task::attempt::Attempt;
use apalis_core::worker::ext::ack::Acknowledge;
use apalis_postgres::{PgAck, PgContext, PgTask};
use serde_json::{Value, json};

use super::{CanaryEnvironment, JobRow, insert_job, register_worker};
use crate::support::apalis_job_id::apalis_job_id;

/// A running row locked by the canary worker, as the wrapper would see it.
fn locked_task(id: &str, worker: &str, max_attempts: i32, attempt: usize) -> PgTask<Value> {
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

#[test]
fn acknowledgement_preserves_scheduling_state() {
    let environment = CanaryEnvironment::shared();
    let pool = environment.pool.clone();
    let queue = "canary.ack";
    let worker = "canary-ack-worker";
    let id = apalis_job_id();
    let metadata = json!({ "wildside.retry.dead_reason": "exhausted", "keep": "marker" });

    environment.run(async {
        register_worker(&pool, worker).await;
        insert_job(
            &pool,
            JobRow::new(queue, &id, "Running")
                .attempts(2)
                .max_attempts(7)
                .run_at_offset_seconds(3_600.0)
                .metadata(metadata.clone())
                .locked_by(worker),
        )
        .await;
    });

    // Capture the exact persisted deadline so the post-acknowledgement check
    // can prove it was left untouched, not merely that it is still in the
    // future.
    let run_at_before: i64 = environment
        .run(async {
            sqlx::query_scalar(
                "SELECT (extract(epoch FROM run_at) * 1000000)::bigint
                 FROM apalis.jobs WHERE id = $1",
            )
            .bind(&id)
            .fetch_one(&pool)
            .await
        })
        .expect("read the scheduled deadline before acknowledgement");

    environment.run(async {
        let task = locked_task(&id, worker, 7, 2);
        let result: Result<(), BoxDynError> = Err(Box::new(ProbeFailure));
        PgAck::new(pool.clone())
            .ack(&result, &task.parts)
            .await
            .expect("acknowledge the running job");
    });

    let row: (String, i32, i32, Option<String>, bool, i64) = environment
        .run(async {
            sqlx::query_as(
                "SELECT status, attempts, max_attempts, metadata::text, run_at > now(),
                        (extract(epoch FROM run_at) * 1000000)::bigint
                 FROM apalis.jobs WHERE id = $1",
            )
            .bind(&id)
            .fetch_one(&pool)
            .await
        })
        .expect("read acknowledged job");

    assert_eq!(row.0, "Failed", "a failure below the cap stays Failed");
    assert_eq!(row.1, 2, "ack records the delivered attempt count");
    assert_eq!(row.2, 7, "ack must not rewrite the attempt cap");
    let stored: Value = serde_json::from_str(row.3.as_deref().expect("metadata retained"))
        .expect("metadata remains valid JSON");
    assert_eq!(stored["wildside.retry.dead_reason"], json!("exhausted"));
    assert_eq!(stored["keep"], json!("marker"));
    assert!(row.4, "ack must preserve the persisted run_at");
    assert_eq!(
        row.5, run_at_before,
        "ack must not rewrite the scheduled deadline"
    );
}

#[test]
fn acknowledgement_keeps_a_shrunk_limit_and_terminal_reason() {
    let environment = CanaryEnvironment::shared();
    let pool = environment.pool.clone();
    let queue = "canary.ack-terminal";
    let worker = "canary-ack-terminal-worker";
    let id = apalis_job_id();

    environment.run(async {
        register_worker(&pool, worker).await;
        insert_job(
            &pool,
            JobRow::new(queue, &id, "Running")
                .attempts(3)
                .max_attempts(3)
                .metadata(json!({ "wildside.retry.dead_reason": "fatal" }))
                .locked_by(worker),
        )
        .await;

        let task = locked_task(&id, worker, 3, 3);
        let result: Result<(), BoxDynError> = Err(Box::new(ProbeFailure));
        PgAck::new(pool.clone())
            .ack(&result, &task.parts)
            .await
            .expect("acknowledge the terminal job");
    });

    let row: (String, Option<String>) = environment
        .run(async {
            sqlx::query_as("SELECT status, metadata::text FROM apalis.jobs WHERE id = $1")
                .bind(&id)
                .fetch_one(&pool)
                .await
        })
        .expect("read terminal job");

    assert_eq!(row.0, "Killed", "ack at the cap stores Killed");
    let stored: Value = serde_json::from_str(row.1.as_deref().expect("metadata retained"))
        .expect("metadata remains valid JSON");
    assert_eq!(stored["wildside.retry.dead_reason"], json!("fatal"));
}

#[derive(Debug)]
struct ProbeFailure;

impl std::fmt::Display for ProbeFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("probe failure")
    }
}

impl StdError for ProbeFailure {}
