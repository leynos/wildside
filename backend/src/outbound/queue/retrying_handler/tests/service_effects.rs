//! Scheduling-effect tests for the retry wrapper's persistence contract.
//!
//! When it can prove ownership, the wrapper asks for exactly one guarded write
//! per failed delivery. It writes nothing after a successful handler, and it
//! writes nothing when the ownership proof is missing or the database guard
//! refuses the update. Terminal writes and their cap-lowering fallback are
//! covered by [`super::service_terminal`]. The scheduler is behind
//! [`RetryPersistence`], so these tests observe calls directly instead of
//! inferring them from database state.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use apalis_core::layers::Service;
use rstest::rstest;
use serde_json::json;

use super::super::{RetryFailureContext, handle_failure};
use super::doubles::{
    DeliveryIdentity, FixedJitter, PersistenceCall, QUEUE, RecordingMetrics, RecordingPersistence,
    TASK_ID, TestJob, WORKER, WriteOutcome, service_with, task_with, wrapper_config,
};
use super::failure_run::{ATTEMPT, CONTEXT_MAX_ATTEMPTS, FailureRun};
use crate::domain::jobs::retry::{
    JitterSample, JobErrorKind, JobFailure, JobRetryPolicy, JobRetryPolicyConfig,
};
use crate::domain::ports::RouteRetryOutcome;
use tracing_test::traced_test;

#[tokio::test]
async fn retryable_failure_schedules_exactly_one_write() {
    let run = FailureRun::new(WriteOutcome::Updated, WriteOutcome::Updated);

    run.apply(JobErrorKind::Retryable).await;

    assert_eq!(
        run.calls(),
        vec![PersistenceCall::Schedule {
            task_id: TASK_ID.to_owned(),
            worker_id: WORKER.to_owned(),
            attempt: ATTEMPT,
            delay: Duration::from_millis(10),
        }]
    );
    assert_eq!(run.retries(), vec![RouteRetryOutcome::Scheduled]);
    assert!(run.dead_letters().is_empty());
}

#[tokio::test]
async fn schedule_failure_counts_one_outcome_and_stops_writing() {
    let run = FailureRun::new(WriteOutcome::NotOwned, WriteOutcome::Updated);

    run.apply(JobErrorKind::Retryable).await;

    assert_eq!(run.calls().len(), 1, "no retry write after failure");
    assert_eq!(run.retries(), vec![RouteRetryOutcome::ScheduleFailure]);
}

#[tokio::test]
async fn scheduling_error_counts_one_outcome_and_stops_writing() {
    let run = FailureRun::new(WriteOutcome::Failed, WriteOutcome::Updated);

    run.apply(JobErrorKind::Retryable).await;

    assert_eq!(run.calls().len(), 1, "no retry write after error");
    assert_eq!(run.retries(), vec![RouteRetryOutcome::ScheduleFailure]);
}

/// Build a policy whose jitter can move a delay away from its nominal value.
///
/// The shared [`policy`] helper disables jitter so decision tests stay exact;
/// these cases exist to prove the wrapper applies the sample it is given.
fn jittered_policy() -> JobRetryPolicy {
    JobRetryPolicy::new(JobRetryPolicyConfig {
        max_attempts: NonZeroU32::new(3).expect("positive attempt limit"),
        base_delay: Duration::from_millis(100),
        multiplier: 2.0,
        max_delay: Duration::from_millis(800),
        jitter_fraction: 0.25,
    })
    .expect("valid jittered policy")
}

/// Drive one retryable failure through `policy` with an injected jitter sample
/// and return the delay the wrapper asked the scheduler to persist.
async fn scheduled_delay_for_sample(policy: &JobRetryPolicy, sample: f64) -> Duration {
    let persistence = RecordingPersistence::new(WriteOutcome::Updated, WriteOutcome::Updated);
    let metrics = RecordingMetrics::default();
    let jitter = FixedJitter(JitterSample::new(sample).expect("jitter sample in 0..=1"));
    let context = RetryFailureContext {
        attempt: NonZeroU32::new(ATTEMPT).expect("positive attempt"),
        context_max_attempts: CONTEXT_MAX_ATTEMPTS,
        policy,
        scheduler: &persistence,
        metrics: &metrics,
        jitter: &jitter,
        queue_name: QUEUE,
        task_id: Some(TASK_ID),
        worker_id: Some(WORKER),
    };
    let failure = JobFailure::new(JobErrorKind::Retryable, "probe failure");
    handle_failure(failure, &context)
        .await
        .expect_err("the wrapper asks Apalis to keep the task failed");

    match persistence.calls().as_slice() {
        [PersistenceCall::Schedule { delay, .. }] => *delay,
        other => panic!("expected exactly one schedule write, got {other:?}"),
    }
}

/// Scenario: two deliveries of the same attempt draw distinct jitter samples.
///
/// Invariant: the persisted delay follows the sample, so a non-zero jitter
/// fraction actually spreads retries instead of being ignored.
#[tokio::test]
async fn distinct_jitter_samples_produce_distinct_scheduled_delays() {
    let policy = jittered_policy();

    let lowest = scheduled_delay_for_sample(&policy, 0.0).await;
    let midpoint = scheduled_delay_for_sample(&policy, 0.5).await;
    let highest = scheduled_delay_for_sample(&policy, 1.0).await;

    assert_eq!(
        midpoint,
        Duration::from_millis(100),
        "the midpoint sample leaves the nominal delay unchanged"
    );
    assert_eq!(
        lowest,
        Duration::from_millis(75),
        "a zero sample applies the full negative jitter offset"
    );
    assert_eq!(
        highest,
        Duration::from_millis(125),
        "a one sample applies the full positive jitter offset"
    );
    assert_ne!(lowest, highest, "the two extremes must differ");
}

/// Scenario: a delivery runs through a service configured with a jitter source.
///
/// Invariant: the sample reaches the persisted delay through
/// `RetryingJobService::call`, so a configuration-supplied jitter source
/// shapes real deliveries rather than merely being stored.
#[rstest]
#[case::lowest(0.0, Duration::from_millis(75))]
#[case::midpoint(0.5, Duration::from_millis(100))]
#[case::highest(1.0, Duration::from_millis(125))]
#[tokio::test]
async fn configured_jitter_source_shapes_the_delivered_delay(
    #[case] sample: f64,
    #[case] expected: Duration,
) {
    let persistence = Arc::new(RecordingPersistence::new(
        WriteOutcome::Updated,
        WriteOutcome::Updated,
    ));
    let metrics = Arc::new(RecordingMetrics::default());
    let jitter = Arc::new(FixedJitter(
        JitterSample::new(sample).expect("sample in 0..=1"),
    ));
    let handler = |_job: TestJob, _attempt: NonZeroU32| async {
        Err(JobFailure::new(JobErrorKind::Retryable, "probe failure"))
    };
    let config = wrapper_config(
        jittered_policy(),
        Arc::clone(&persistence),
        Arc::clone(&metrics),
    )
    .with_jitter(jitter);
    let mut service = service_with(config, handler);
    let task = task_with(
        json!({"v": "v1", "probeId": "probe"}),
        usize::try_from(ATTEMPT).expect("the fixed attempt fits in usize"),
        DeliveryIdentity::new(Some(WORKER), Some(WORKER)),
    );

    let result = service.call(task).await;

    assert!(result.is_err(), "the delivery stays failed");
    assert_eq!(
        persistence.calls(),
        vec![PersistenceCall::Schedule {
            task_id: TASK_ID.to_owned(),
            worker_id: WORKER.to_owned(),
            attempt: ATTEMPT,
            delay: expected,
        }],
        "the persisted delay must follow the configured jitter sample"
    );
    assert_eq!(metrics.retries(), vec![RouteRetryOutcome::Scheduled]);
}

#[traced_test]
#[tokio::test]
async fn schedule_failure_warns_with_the_failure_decision() {
    let run = FailureRun::new(WriteOutcome::NotOwned, WriteOutcome::Updated);

    run.apply(JobErrorKind::Retryable).await;

    assert!(logs_contain("schedule_failure"));
}

#[traced_test]
#[tokio::test]
async fn a_scheduled_retry_reports_at_info_not_warn() {
    let run = FailureRun::new(WriteOutcome::Updated, WriteOutcome::Updated);

    run.apply(JobErrorKind::Retryable).await;

    logs_assert(|lines: &[&str]| {
        let scheduled = lines
            .iter()
            .find(|line| line.contains("job failure scheduled for durable retry"))
            .ok_or_else(|| "the scheduled retry must log its outcome".to_owned())?;
        if !scheduled.contains("INFO") {
            return Err(format!(
                "a scheduled retry must log at INFO so warnings mean refused writes: {scheduled}"
            ));
        }
        if lines
            .iter()
            .any(|line| line.contains("WARN") && line.contains("scheduled for durable retry"))
        {
            return Err("a scheduled retry must not also log at WARN".to_owned());
        }
        Ok(())
    });
}

#[rstest::rstest]
#[case::retryable_missing_task_id(JobErrorKind::Retryable, None, Some(WORKER))]
#[case::retryable_missing_worker_id(JobErrorKind::Retryable, Some(TASK_ID), None)]
#[case::retryable_missing_both(JobErrorKind::Retryable, None, None)]
#[case::terminal_missing_task_id(JobErrorKind::Fatal, None, Some(WORKER))]
#[case::terminal_missing_worker_id(JobErrorKind::Fatal, Some(TASK_ID), None)]
#[case::terminal_missing_both(JobErrorKind::Fatal, None, None)]
#[tokio::test]
async fn missing_identity_writes_nothing(
    #[case] kind: JobErrorKind,
    #[case] task_id: Option<&'static str>,
    #[case] worker_id: Option<&'static str>,
) {
    let run = FailureRun::new(WriteOutcome::Updated, WriteOutcome::Updated);

    run.apply_with_identity(kind, task_id, worker_id).await;

    assert!(
        run.calls().is_empty(),
        "the wrapper must not persist without an owning task and worker"
    );
    assert!(
        run.dead_letters().is_empty(),
        "an unwritten outcome is not a dead letter"
    );
    assert!(
        run.terminal_write_failures().is_empty(),
        "an absent identity is not a refused terminal write"
    );
    if kind == JobErrorKind::Retryable {
        assert_eq!(
            run.retries(),
            vec![RouteRetryOutcome::ScheduleFailure],
            "an unpersisted retry is a scheduling failure"
        );
    } else {
        assert!(run.retries().is_empty());
    }
}

#[traced_test]
#[tokio::test]
async fn missing_identity_logs_the_blocked_metadata() {
    let run = FailureRun::new(WriteOutcome::Updated, WriteOutcome::Updated);

    run.apply_with_identity(JobErrorKind::Retryable, None, Some(WORKER))
        .await;

    assert!(logs_contain("retry metadata missing"));
}
