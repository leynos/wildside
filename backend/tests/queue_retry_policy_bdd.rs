//! Behaviour tests for durable retry scheduling through Apalis PostgreSQL.

#[path = "queue_retry_policy_bdd/mod.rs"]
mod retry_policy_world;
include!("support/entrypoint.rs");
declare_test_support!(atexit_cleanup, embedded_postgres, queue_retry_worker);

use std::time::Duration;

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use serde_json::json;

pub(crate) use retry_policy_world::{
    QueueRetryPolicyWorld, RetryHandlerMode, assert_deadlines_advance,
};

#[fixture]
fn world() -> QueueRetryPolicyWorld {
    QueueRetryPolicyWorld::new()
}

#[given("a queue with a retry policy of {attempts} attempts and millisecond backoff")]
fn a_queue_with_retry_policy(world: &QueueRetryPolicyWorld, attempts: u32) {
    world.set_policy(attempts);
}

#[given("a queue with a retry policy of one attempt and millisecond backoff")]
fn a_queue_with_one_attempt_retry_policy(world: &QueueRetryPolicyWorld) {
    world.set_policy(1);
}

#[given("a job whose handler fails twice then succeeds")]
fn a_job_that_fails_twice_then_succeeds(world: &QueueRetryPolicyWorld) {
    world.set_handler_mode(RetryHandlerMode::FailRetryableThenSucceed(2));
    world.enqueue_job();
}

#[given("a job whose handler always fails with a retryable error")]
fn a_job_that_always_fails_retryably(world: &QueueRetryPolicyWorld) {
    world.set_handler_mode(RetryHandlerMode::AlwaysRetryable);
    world.enqueue_job();
}

#[given("a job whose handler fails with a fatal error")]
fn a_job_that_fails_fatally(world: &QueueRetryPolicyWorld) {
    world.set_handler_mode(RetryHandlerMode::Fatal);
    world.enqueue_job();
}

#[given("the job has an unrelated metadata marker")]
fn the_job_has_unrelated_metadata(world: &QueueRetryPolicyWorld) {
    world.set_unrelated_metadata();
}

#[given("a pre-existing job permits {attempts} attempts")]
fn a_preexisting_job_permits_attempts(world: &QueueRetryPolicyWorld, attempts: i32) {
    world.insert_preexisting_job(attempts);
}

#[given("its handler always fails with a retryable error")]
fn its_handler_always_fails_retryably(world: &QueueRetryPolicyWorld) {
    world.set_handler_mode(RetryHandlerMode::AlwaysRetryable);
}

#[given("a stored job whose body is malformed")]
fn a_stored_job_with_a_malformed_body(world: &QueueRetryPolicyWorld) {
    world.insert_raw_job(json!({"v": "v1"}));
}

#[given("a stored job whose envelope version is unsupported")]
fn a_stored_job_with_an_unsupported_version(world: &QueueRetryPolicyWorld) {
    let payload = json!({"v": "v2", "probeId": world.probe_id()});
    world.insert_raw_job(payload);
}

#[when("the worker processes the queue")]
fn the_worker_processes_the_queue(world: &QueueRetryPolicyWorld) {
    world.run_worker();
}

#[when("the worker polls the queue for a bounded window")]
fn the_worker_polls_the_queue_for_a_bounded_window(world: &QueueRetryPolicyWorld) {
    world.run_worker_for_polls(100);
}

#[when("a job is enqueued through the route queue adapter")]
fn a_job_is_enqueued_through_the_route_queue_adapter(world: &QueueRetryPolicyWorld) {
    world.enqueue_job();
}

#[then("the handler runs three times with attempts 1, 2 and 3")]
fn the_handler_runs_three_times_with_increasing_attempts(world: &QueueRetryPolicyWorld) {
    assert_eq!(world.observed_attempts(), vec![1, 2, 3]);
}

#[then("the handler runs exactly {expected} times")]
fn the_handler_runs_exactly_times(world: &QueueRetryPolicyWorld, expected: usize) {
    assert_eq!(world.observed_attempts().len(), expected);
}

#[then("the handler runs exactly once")]
fn the_handler_runs_once(world: &QueueRetryPolicyWorld) {
    assert_eq!(world.observed_attempts().len(), 1);
}

#[then("each rerun starts no earlier than its persisted retry time")]
fn each_rerun_starts_after_its_persisted_retry_time(world: &QueueRetryPolicyWorld) {
    let observations = world.observations();
    assert!(observations.len() > 1, "the handler must be retried");
    for observation in observations.iter().skip(1) {
        assert!(
            observation.started_at_micros >= observation.run_at_micros,
            "attempt {} started before its persisted run_at",
            observation.attempt
        );
    }

    // A no-op or swallowed scheduling write would leave `run_at` unchanged and
    // still satisfy the check above whenever the row is already due. Requiring
    // every adjacent pair to advance by at least the base delay makes the
    // durable-backoff claim non-vacuous, including the final pair: the last
    // attempt observes the deadline the previous failure wrote.
    assert_deadlines_advance(&observations, world.base_delay_micros());
}

#[then("the job completes with status {status}")]
fn the_job_completes_with_status(world: &QueueRetryPolicyWorld, status: String) {
    assert_eq!(world.job_row().status, status.trim_matches('"'));
}

#[then("the acknowledgement preserves the final attempt state")]
fn the_acknowledgement_preserves_final_attempt_state(world: &QueueRetryPolicyWorld) {
    world.assert_acknowledged_state();
}

#[then("unrelated metadata survives acknowledgement")]
fn unrelated_metadata_survives_acknowledgement(world: &QueueRetryPolicyWorld) {
    assert_eq!(
        world.job_row().unrelated_metadata.as_deref(),
        Some("retain-me")
    );
}

#[then(
    "retry metrics contain {retries} scheduled, {exhausted} exhausted and {fatal} fatal outcomes"
)]
fn retry_metrics_record_bounded_outcomes(
    world: &QueueRetryPolicyWorld,
    retries: usize,
    exhausted: usize,
    fatal: usize,
) {
    world.assert_metrics(retries, exhausted, fatal);
}

#[then("the row is stored as {status} and satisfies the dead predicate")]
fn the_row_is_dead_with_status(world: &QueueRetryPolicyWorld, status: String) {
    let row = world.job_row();
    assert_eq!(row.status, status.trim_matches('"'));
    assert!(
        row.is_dead(),
        "row should satisfy the dead predicate: {row:?}"
    );
}

#[then("the persisted terminal reason is {reason}")]
fn the_persisted_terminal_reason_is(world: &QueueRetryPolicyWorld, reason: String) {
    assert_eq!(world.job_row().dead_reason(), reason.trim_matches('"'));
}

#[then("further polling delivers no more runs")]
fn further_polling_does_not_redeliver(world: &QueueRetryPolicyWorld) {
    let report = world.worker_report();
    assert!(report.reached_terminal, "worker stopped at its hard cap");
    assert!(
        report.quiet_period_elapsed,
        "worker did not complete its quiet poll"
    );
    assert_eq!(report.runs_at_terminal, report.runs_after_quiet);
}

#[then("the stored job allows at most {attempts} attempts")]
fn the_stored_job_has_retry_budget(world: &QueueRetryPolicyWorld, attempts: i32) {
    assert_eq!(world.job_row().max_attempts, attempts);
}

#[then("the handler never runs")]
fn the_handler_never_runs(world: &QueueRetryPolicyWorld) {
    assert_eq!(
        world.observed_attempts(),
        Vec::<u32>::new(),
        "a payload rejected at the queue boundary must not reach the handler"
    );
}

#[then("the latest job row satisfies the dead predicate")]
fn the_latest_job_row_is_dead(world: &QueueRetryPolicyWorld) {
    let row = world.latest_job_row();
    assert!(
        row.is_dead(),
        "rejected payload should be storage-dead: {row:?}"
    );
}

#[then("the latest job row records the terminal reason {reason}")]
fn the_latest_job_row_records_terminal_reason(world: &QueueRetryPolicyWorld, reason: String) {
    assert_eq!(
        world.latest_job_row().dead_reason(),
        reason.trim_matches('"')
    );
}

#[test]
fn worker_deadline_bounds_a_non_completing_handler() {
    let world = QueueRetryPolicyWorld::new();
    world.set_policy(3);
    world.set_handler_mode(RetryHandlerMode::NeverCompletes);
    world.enqueue_job();

    let error = world
        .run_worker_with_deadline(Duration::from_secs(2))
        .expect_err("a handler that never completes must hit the harness deadline");
    assert!(
        error.contains("deadline"),
        "deadline failure should name its cause: {error}"
    );
    // The handler records its invocation before blocking forever, so this
    // proves the deadline ended a run that was genuinely in progress rather
    // than one that failed to start.
    assert_eq!(
        world.observed_attempts(),
        vec![1],
        "the deadline must interrupt a running handler"
    );
}

#[scenario(
    path = "tests/features/queue_retry_policy.feature",
    name = "A transiently failing job is retried before it succeeds"
)]
fn transient_failure_retries_then_succeeds(world: QueueRetryPolicyWorld) {
    drop(world);
}

#[scenario(
    path = "tests/features/queue_retry_policy.feature",
    name = "A retryable failure is dead-lettered after exhausting attempts"
)]
fn retryable_failure_is_dead_lettered_after_exhaustion(world: QueueRetryPolicyWorld) {
    drop(world);
}

#[scenario(
    path = "tests/features/queue_retry_policy.feature",
    name = "A fatal failure is dead-lettered immediately"
)]
fn fatal_failure_is_dead_lettered_immediately(world: QueueRetryPolicyWorld) {
    drop(world);
}

#[scenario(
    path = "tests/features/queue_retry_policy.feature",
    name = "A fatal failure at the attempt limit stores Killed"
)]
fn fatal_failure_at_attempt_limit_stores_killed(world: QueueRetryPolicyWorld) {
    drop(world);
}

#[scenario(
    path = "tests/features/queue_retry_policy.feature",
    name = "A lower policy limit dead-letters a pre-existing job"
)]
fn lower_policy_limit_dead_letters_preexisting_job(world: QueueRetryPolicyWorld) {
    drop(world);
}

#[scenario(
    path = "tests/features/queue_retry_policy.feature",
    name = "An enqueued job carries the queue retry budget"
)]
fn enqueued_job_carries_queue_retry_budget(world: QueueRetryPolicyWorld) {
    drop(world);
}

#[scenario(
    path = "tests/features/queue_retry_policy.feature",
    name = "A malformed stored job is dead-lettered without invoking the handler"
)]
fn malformed_job_is_dead_lettered_without_invoking_the_handler(world: QueueRetryPolicyWorld) {
    drop(world);
}

#[scenario(
    path = "tests/features/queue_retry_policy.feature",
    name = "An unsupported envelope version is dead-lettered without invoking the handler"
)]
fn unsupported_version_is_dead_lettered_without_invoking_the_handler(world: QueueRetryPolicyWorld) {
    drop(world);
}
