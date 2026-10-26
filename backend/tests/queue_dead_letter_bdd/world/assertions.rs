//! Behaviour assertions for dead-letter operations.

use serde_json::Value;

use backend::domain::ports::{DeadLetterError, DeadLetterQueue, DeadReason};

use super::QueueDeadLetterWorld;

impl QueueDeadLetterWorld {
    pub fn assert_failure_context(&self) {
        let id = self.current_id();
        let listing = self
            .runtime
            .block_on(self.dead_letters.list_dead(100))
            .expect("list dead jobs");
        let job = listing
            .iter()
            .find(|job| job.id == id)
            .expect("listed probe job");
        assert_eq!(job.reason, DeadReason::Exhausted);
        assert_eq!(job.attempts, 3);
        assert_eq!(
            job.last_error.as_deref(),
            Some("temporary dead-letter probe failure")
        );
    }

    pub fn assert_status(&self, expected: &str) {
        assert_eq!(self.snapshot(&self.current_id()).status, expected);
    }

    pub fn assert_requeue_count(&self, expected: u32) {
        let snapshot = self.snapshot(&self.current_id());
        let count = snapshot
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get("wildside.retry.requeue_count"))
            .and_then(Value::as_u64);
        assert_eq!(count, Some(u64::from(expected)));
    }

    pub fn assert_requeue_restored_state(&self) {
        self.take_operation_result()
            .expect("dead-job requeue succeeds");
        let snapshot = self.snapshot(&self.current_id());
        assert_eq!(snapshot.status, "Pending");
        assert_eq!(snapshot.attempts, 0);
        assert_eq!(snapshot.max_attempts, Some(3));
        assert!(snapshot.run_at <= chrono::Utc::now());
        assert_eq!(snapshot.lock_by, None);
        assert_eq!(snapshot.lock_at, None);
        let metadata = snapshot.metadata.expect("requeue metadata");
        assert_eq!(metadata["wildside.retry.requeue_count"], 5);
        assert!(metadata.get("wildside.retry.dead_reason").is_none());
        assert_eq!(metadata["operator.note"], "preserve-me");
    }

    pub fn assert_no_handler_runs(&self) {
        assert_eq!(*self.handler_calls.lock().expect("handler calls mutex"), 0);
    }

    pub fn assert_unclassified(&self) {
        let id = self.current_id();
        let listing = self
            .runtime
            .block_on(self.dead_letters.list_dead(100))
            .expect("list dead jobs");
        let job = listing
            .iter()
            .find(|job| job.id == id)
            .expect("legacy dead job listed");
        assert_eq!(job.reason, DeadReason::Unclassified);
        assert_eq!(job.last_error, None);
    }

    pub fn assert_full_retry_budget_after_requeue(&self) {
        assert_eq!(*self.handler_calls.lock().expect("handler calls mutex"), 3);
    }

    pub fn assert_exhausted_after_requeue(&self) {
        self.assert_failure_context();
    }

    pub fn assert_exhausted_reason_replaced_fatal(&self) {
        let snapshot = self.snapshot(&self.current_id());
        assert_eq!(
            snapshot
                .metadata
                .as_ref()
                .and_then(|metadata| metadata["wildside.retry.dead_reason"].as_str()),
            Some("exhausted")
        );
    }

    pub fn assert_not_dead_error(&self) {
        assert!(matches!(
            self.take_operation_result(),
            Err(DeadLetterError::NotDead { .. })
        ));
    }

    pub fn assert_state_unchanged(&self) {
        let before = self
            .state_before_operation
            .lock()
            .expect("snapshot mutex")
            .take()
            .expect("pre-operation state");
        assert_eq!(self.snapshot(&self.current_id()), before);
    }

    pub fn assert_listing_empty(&self) {
        self.list_dead(100);
        assert!(self.listing.lock().expect("listing mutex").is_empty());
    }

    pub fn assert_job_absent(&self) {
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

    pub fn assert_only_newest_job_listed(&self) {
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
        assert_eq!(listing.len(), 1);
        assert_eq!(listing[0].id, expected);
    }

    pub fn assert_unknown_queue_error(&self) {
        assert!(matches!(
            self.take_operation_result(),
            Err(DeadLetterError::UnknownQueue { .. })
        ));
    }
}
