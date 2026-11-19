//! Behaviour assertions for dead-letter operations.

use chrono::{DateTime, Utc};
use serde_json::Value;

use backend::domain::ports::{DeadJob, DeadJobSkip, DeadLetterError, DeadLetterQueue, DeadReason};

use super::QueueDeadLetterWorld;

impl QueueDeadLetterWorld {
    /// Read the current job's entry from a fresh dead-job listing.
    fn listed_current_job(&self) -> DeadJob {
        let id = self.current_id();
        let listing = self
            .runtime
            .block_on(self.dead_letters.list_dead(100))
            .expect("list dead jobs");
        listing
            .into_parts()
            .0
            .into_iter()
            .find(|job| job.id == id)
            .expect("listed probe job")
    }

    pub(crate) fn assert_failure_context(&self) {
        let job = self.listed_current_job();
        assert_eq!(job.reason, DeadReason::Exhausted);
        assert_eq!(job.attempts, 3);
        assert_eq!(
            job.last_error.as_deref(),
            Some("temporary dead-letter probe failure")
        );
    }

    pub(crate) fn assert_status(&self, expected: &str) {
        assert_eq!(self.snapshot(&self.current_id()).status, expected);
    }

    pub(crate) fn assert_requeue_count(&self, expected: u32) {
        let snapshot = self.snapshot(&self.current_id());
        let count = snapshot
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("wildside.retry.requeue_count"))
            .and_then(Value::as_u64);
        assert_eq!(count, Some(u64::from(expected)));
    }

    pub(crate) fn assert_invalid_job_data_error(&self) {
        assert!(matches!(
            self.take_operation_result(),
            Err(DeadLetterError::InvalidJobData { .. })
        ));
    }

    pub(crate) fn assert_requeue_restored_state(&self) {
        self.take_operation_result()
            .expect("dead-job requeue succeeds");
        let snapshot = self.snapshot(&self.current_id());
        assert_eq!(snapshot.status, "Pending");
        assert_eq!(snapshot.attempts, 0);
        assert_eq!(snapshot.max_attempts, Some(3));
        // Requeue writes `run_at` from the database clock, and the fetch
        // predicate compares it with that same clock. Reading the deadline
        // through the application clock would mix two clocks: a database clock
        // behind the application's would let a future deadline look due.
        let database_now = self.database_now();
        assert!(
            is_due(snapshot.run_at, database_now),
            "a requeued row is immediately eligible, not scheduled for later"
        );
        self.assert_future_deadline_is_not_due();
        assert_eq!(snapshot.lock_by, None);
        assert_eq!(snapshot.lock_at, None);
        let metadata = snapshot.metadata.expect("requeue metadata");
        assert_eq!(metadata["wildside.retry.requeue_count"], 5);
        assert!(metadata.get("wildside.retry.dead_reason").is_none());
        assert_eq!(metadata["operator.note"], "preserve-me");
    }

    /// Assert a requeue replaced a future deadline with an eligible one.
    ///
    /// The seed advances the deadline an hour past the database clock, and the
    /// assertion first requires that deadline to be ineligible. That is the
    /// negative control for the requeue write: a requeue that left `run_at`
    /// alone, or replaced it with another future value, keeps the row
    /// ineligible and fails the second assertion. Both readings pair the
    /// deadline with the database clock in one statement, because that is the
    /// comparison Apalis's fetch predicate performs.
    pub(crate) fn assert_requeued_deadline_is_immediately_eligible(&self) {
        self.take_operation_result()
            .expect("dead-job requeue succeeds");
        let before = self
            .deadline_before_requeue
            .lock()
            .expect("pre-requeue deadline mutex")
            .take()
            .expect("pre-requeue deadline reading");
        assert!(
            !is_due(before.run_at, before.database_now),
            "the seed must store a deadline the database has not reached, else the \
             requeue write would be unobservable: {:?}",
            before
        );
        let after = self.read_deadline_and_clock(&self.current_id());
        assert!(
            is_due(after.run_at, after.database_now),
            "requeue must make the restored row immediately eligible: stored run_at {:?}, \
             database now {:?}",
            after.run_at,
            after.database_now
        );
    }

    /// Read the pool's clock, the one the fetch predicate compares with.
    fn database_now(&self) -> DateTime<Utc> {
        self.runtime
            .block_on(sqlx::query_scalar::<_, DateTime<Utc>>("SELECT now()").fetch_one(&self.pool))
            .expect("read the database clock")
    }

    /// Show that the eligibility check rejects a future database deadline.
    ///
    /// A positive assertion alone cannot distinguish a working check from one
    /// that accepts everything, so the same predicate is applied to a deadline
    /// it must refuse. The staged deadline is written inside a transaction that
    /// is rolled back, so the row the scenario observes keeps its requeued
    /// values, and both readings come from the transaction's clock.
    fn assert_future_deadline_is_not_due(&self) {
        let id = self.current_id();
        self.runtime.block_on(async {
            let mut transaction = self.pool.begin().await.expect("begin deadline probe");
            sqlx::query("UPDATE apalis.jobs SET run_at = now() + interval '1 hour' WHERE id = $1")
                .bind(id.as_str())
                .execute(&mut *transaction)
                .await
                .expect("stage a future deadline");
            let (run_at, database_now): (DateTime<Utc>, DateTime<Utc>) =
                sqlx::query_as("SELECT run_at, now() FROM apalis.jobs WHERE id = $1")
                    .bind(id.as_str())
                    .fetch_one(&mut *transaction)
                    .await
                    .expect("read the staged deadline");
            transaction
                .rollback()
                .await
                .expect("roll back the deadline probe");
            assert!(
                !is_due(run_at, database_now),
                "a future database deadline must not count as due"
            );
        });
    }

    pub(crate) fn assert_legacy_row_not_fetched(&self) {
        let snapshot = self.snapshot(&self.current_id());
        assert_eq!(snapshot.status, "Failed");
        assert_eq!(snapshot.attempts, 3);
        assert_eq!(snapshot.max_attempts, None);
    }

    pub(crate) fn assert_control_row_processed(&self) {
        let id = self
            .poll_control_job_id
            .lock()
            .expect("poll control job id mutex")
            .clone()
            .expect("poll control row id");
        assert_eq!(self.snapshot(&id).status, "Done");
        assert_eq!(*self.handler_calls.lock().expect("handler calls mutex"), 1);
        assert!(self.worker_was_registered());
    }

    pub(crate) fn assert_unclassified(&self) {
        let job = self.listed_current_job();
        assert_eq!(job.reason, DeadReason::Unclassified);
        assert_eq!(job.last_error, None);
    }

    pub(crate) fn assert_fatal_failure_context(&self) {
        let job = self.listed_current_job();
        assert_eq!(job.reason, DeadReason::Fatal);
        assert_eq!(job.attempts, 1);
        assert_eq!(
            job.last_error.as_deref(),
            Some("permanent dead-letter probe failure")
        );
    }

    pub(crate) fn assert_full_retry_budget_after_requeue(&self) {
        assert_eq!(*self.handler_calls.lock().expect("handler calls mutex"), 3);
    }

    pub(crate) fn assert_exhausted_after_requeue(&self) {
        self.assert_failure_context();
    }

    pub(crate) fn assert_exhausted_reason_replaced_fatal(&self) {
        let snapshot = self.snapshot(&self.current_id());
        assert_eq!(
            snapshot
                .metadata
                .as_ref()
                .and_then(|metadata| metadata["wildside.retry.dead_reason"].as_str()),
            Some("exhausted")
        );
    }

    pub(crate) fn assert_not_dead_error(&self) {
        assert!(matches!(
            self.take_operation_result(),
            Err(DeadLetterError::NotDead { .. })
        ));
    }

    pub(crate) fn assert_state_unchanged(&self) {
        let before = self
            .state_before_operation
            .lock()
            .expect("snapshot mutex")
            .take()
            .expect("pre-operation state");
        assert_eq!(self.snapshot(&self.current_id()), before);
    }

    pub(crate) fn assert_listing_empty(&self) {
        self.list_dead(100);
        let listing = self.listing.lock().expect("listing mutex");
        assert!(listing.jobs().is_empty());
        assert!(
            listing.skipped().is_empty(),
            "an empty listing has nothing it could not read: {:?}",
            listing.skipped()
        );
    }

    /// Assert the listing reported the one row it could not read.
    pub(crate) fn assert_listing_skips_the_negative_attempt_row(&self) {
        let listing = self.listing.lock().expect("dead-job listing mutex");
        assert!(
            listing.jobs().is_empty(),
            "a row with a negative attempt count is not listable"
        );
        let skipped = listing.skipped();
        assert_eq!(skipped.len(), 1, "the one malformed row is reported");
        assert_eq!(skipped[0].id, self.current_id());
        assert_eq!(skipped[0].cause, DeadJobSkip::NegativeAttempts);
    }

    pub(crate) fn assert_listing_contains_only_valid_dead_job(&self) {
        let valid_id = self
            .newest_job_id
            .lock()
            .expect("newest job id mutex")
            .clone()
            .expect("valid dead job id");
        let listing = self.listing.lock().expect("dead-job listing mutex");
        assert_eq!(listing.jobs().len(), 1);
        assert_eq!(listing.jobs()[0].id, valid_id);
        let skipped = listing.skipped();
        assert!(
            !skipped.is_empty(),
            "the unreadable rows are reported rather than dropped"
        );
        assert!(
            skipped
                .iter()
                .all(|row| row.cause == DeadJobSkip::InvalidRequeueCount),
            "every rejected row names the field that made it unreadable: {skipped:?}"
        );
    }

    pub(crate) fn assert_job_absent(&self) {
        let exists = self
            .runtime
            .block_on(
                sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS (SELECT 1 FROM apalis.jobs WHERE id = $1)",
                )
                .bind(self.current_id().as_str())
                .fetch_one(&self.pool),
            )
            .expect("check discarded row");
        assert!(!exists);
    }

    pub(crate) fn assert_only_newest_job_listed(&self) {
        let expected = self
            .newest_job_id
            .lock()
            .expect("newest id mutex")
            .clone()
            .expect("newest job id");
        let listing = self
            .runtime
            .block_on(self.dead_letters.list_dead(1))
            .expect("list newest dead job");
        assert_eq!(listing.jobs().len(), 1);
        assert_eq!(listing.jobs()[0].id, expected);
    }

    pub(crate) fn assert_unknown_queue_error(&self) {
        assert!(matches!(
            self.take_operation_result(),
            Err(DeadLetterError::UnknownQueue { .. })
        ));
    }
}

/// Whether a stored deadline is already due under one clock.
///
/// The stored `run_at` and the comparison point have to come from the same
/// clock, because Apalis's fetch predicate evaluates both in PostgreSQL.
fn is_due(run_at: DateTime<Utc>, database_now: DateTime<Utc>) -> bool {
    run_at <= database_now
}
