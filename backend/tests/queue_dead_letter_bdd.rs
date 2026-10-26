//! Behaviour tests for dead-letter listing, requeue, and discard.

#[path = "queue_dead_letter_bdd/mod.rs"]
mod queue_dead_letter_world;
include!("support/entrypoint.rs");
declare_test_support!(atexit_cleanup, embedded_postgres, queue_retry_worker);

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

pub use queue_dead_letter_world::QueueDeadLetterWorld;

#[fixture]
fn world() -> QueueDeadLetterWorld {
    QueueDeadLetterWorld::new()
}

#[given("a job has been dead-lettered after exhausting its retries")]
fn a_job_has_been_dead_lettered_after_exhaustion(world: &QueueDeadLetterWorld) {
    world.exhaust_job();
}

#[given("the underlying fault has been fixed")]
fn the_underlying_fault_has_been_fixed(world: &QueueDeadLetterWorld) {
    world.fix_fault();
}

#[given("a disposable legacy schema permits a null max_attempts value")]
fn the_legacy_schema_permits_null_attempt_limits(world: &QueueDeadLetterWorld) {
    world.allow_null_attempt_limit();
}

#[given("a failed row has 3 attempts, below Apalis's null fallback of 25")]
fn a_legacy_failed_row_has_three_attempts(world: &QueueDeadLetterWorld) {
    world.insert_legacy_failed_row(3);
}

#[given("a job has been dead-lettered by a fatal error")]
fn a_job_has_been_dead_lettered_by_fatal_error(world: &QueueDeadLetterWorld) {
    world.fatal_job();
}

#[given("the dead job has an existing requeue count and operator metadata")]
fn the_dead_job_has_existing_requeue_metadata(world: &QueueDeadLetterWorld) {
    world.add_requeue_metadata();
}

#[given("the job now fails with a retryable error")]
fn the_job_now_fails_retryably(world: &QueueDeadLetterWorld) {
    world.fail_retryably();
}

#[given("a pending job exists")]
fn a_pending_job_exists(world: &QueueDeadLetterWorld) {
    world.insert_pending_job();
}

#[given("a pending job and a running job exist")]
fn pending_and_running_jobs_exist(world: &QueueDeadLetterWorld) {
    world.insert_pending_and_running_jobs();
}

#[given("a failed dead row has no recognized terminal reason")]
fn a_failed_dead_row_has_no_recognized_reason(world: &QueueDeadLetterWorld) {
    world.insert_unclassified_dead_row();
}

#[given("two dead jobs exist")]
fn two_dead_jobs_exist(world: &QueueDeadLetterWorld) {
    world.insert_two_dead_jobs();
}

#[given("a dead job belongs to an unconfigured queue")]
fn a_dead_job_belongs_to_an_unconfigured_queue(world: &QueueDeadLetterWorld) {
    world.insert_unconfigured_dead_job();
}

#[when("the operator lists dead jobs")]
fn the_operator_lists_dead_jobs(world: &QueueDeadLetterWorld) {
    world.list_dead(100);
}

#[when("the operator lists dead jobs with a limit of {limit:u32}")]
fn the_operator_lists_dead_jobs_with_limit(world: &QueueDeadLetterWorld, limit: u32) {
    world.list_dead(limit);
}

#[when("the operator requeues the dead job")]
fn the_operator_requeues_the_dead_job(world: &QueueDeadLetterWorld) {
    world.requeue_dead_job();
}

#[when("the operator requeues that job")]
fn the_operator_requeues_that_job(world: &QueueDeadLetterWorld) {
    world.requeue_current_job();
}

#[when("the operator discards that job")]
fn the_operator_discards_that_job(world: &QueueDeadLetterWorld) {
    world.discard_current_job();
}

#[when("the operator discards the dead job")]
fn the_operator_discards_the_dead_job(world: &QueueDeadLetterWorld) {
    world.discard_dead_job();
}

#[when("the worker processes the queue")]
fn the_worker_processes_the_queue(world: &QueueDeadLetterWorld) {
    world.process_queue();
}

#[then("the listing contains the job with its reason, attempt count and last error")]
fn the_listing_contains_failure_context(world: &QueueDeadLetterWorld) {
    world.assert_failure_context();
}

#[then("the job completes with status {status}")]
fn the_job_completes_with_status(world: &QueueDeadLetterWorld, status: String) {
    world.assert_status(&status);
}

#[then("the job retains one requeue in its metadata")]
fn the_job_retains_one_requeue_in_metadata(world: &QueueDeadLetterWorld) {
    world.assert_requeue_count(1);
}

#[then("Apalis does not fetch the row")]
fn apalis_does_not_fetch_the_row(world: &QueueDeadLetterWorld) {
    world.assert_no_handler_runs();
}

#[then("the dead listing contains it as Unclassified")]
fn the_dead_listing_contains_unclassified(world: &QueueDeadLetterWorld) {
    world.assert_unclassified();
}

#[then("the handler runs the queue's full number of attempts")]
fn the_handler_runs_the_full_retry_budget(world: &QueueDeadLetterWorld) {
    world.assert_full_retry_budget_after_requeue();
}

#[then("the job is dead because its retries were exhausted")]
fn the_job_is_dead_because_retries_were_exhausted(world: &QueueDeadLetterWorld) {
    world.assert_exhausted_after_requeue();
}

#[then("the requeue restores its budget and preserves unrelated metadata")]
fn the_requeue_restores_budget_and_metadata(world: &QueueDeadLetterWorld) {
    world.assert_requeue_restored_state();
}

#[then("the fatal reason has been replaced with exhausted")]
fn the_fatal_reason_has_been_replaced(world: &QueueDeadLetterWorld) {
    world.assert_exhausted_reason_replaced_fatal();
}

#[then("the requeue count remains one")]
fn the_requeue_count_remains_one(world: &QueueDeadLetterWorld) {
    world.assert_requeue_count(1);
}

#[then("the operation fails because the job is not dead")]
fn the_operation_fails_because_the_job_is_not_dead(world: &QueueDeadLetterWorld) {
    world.assert_not_dead_error();
}

#[then("the job's state is unchanged")]
fn the_jobs_state_is_unchanged(world: &QueueDeadLetterWorld) {
    world.assert_state_unchanged();
}

#[then("the listing is empty")]
fn the_listing_is_empty(world: &QueueDeadLetterWorld) {
    world.assert_listing_empty();
}

#[then("the dead job row is gone")]
fn the_dead_job_row_is_gone(world: &QueueDeadLetterWorld) {
    world.assert_job_absent();
}

#[then("only the newest dead job is listed")]
fn only_the_newest_dead_job_is_listed(world: &QueueDeadLetterWorld) {
    world.assert_only_newest_job_listed();
}

#[then("the operation fails because its queue has no retry budget")]
fn the_operation_fails_for_unknown_queue(world: &QueueDeadLetterWorld) {
    world.assert_unknown_queue_error();
}

#[scenario(path = "tests/features/queue_dead_letter.feature")]
fn dead_letter_behaviour(world: QueueDeadLetterWorld) {}

#[test]
fn dead_letter_listing_plan_uses_a_bounded_index_path() {
    let world = QueueDeadLetterWorld::new();
    world.assert_listing_plan_avoids_sequential_scan();
}
