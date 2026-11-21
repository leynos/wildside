//! Scenario seeds for dead, legacy, and malformed Apalis rows.

use backend::domain::ports::DeadJobId;

use super::rows::{JobSeed, JobSnapshot};
use super::{QUEUE_NAME, QueueDeadLetterWorld};

/// Rows seeded beside a discard target, recorded before the operation runs.
///
/// A discard names one identifier. Reading back only the target's absence
/// cannot distinguish deleting that row from deleting every dead row, and
/// cannot see a live row caught in the same delete. The controls therefore
/// carry the live row's seeded state for comparison and the unrelated dead
/// row's identifier for the listing assertion.
pub(super) struct DiscardControls {
    pub(super) live_job_id: DeadJobId,
    pub(super) live_state_before: JobSnapshot,
    pub(super) unrelated_dead_job_id: DeadJobId,
}

impl QueueDeadLetterWorld {
    /// Insert an exhausted-shaped dead row and make it the current job.
    ///
    /// The scenarios that build a single dead row differ only in which queue
    /// it belongs to and what the stored failure says, so those details are
    /// parameters rather than a copy of the seed.
    fn insert_current_failed_dead_row(
        &self,
        job_type: &str,
        reason: &str,
        last_error: Option<&str>,
    ) {
        let id = self.insert_job(JobSeed {
            job_type,
            status: "Failed",
            attempts: 3,
            max_attempts: Some(3),
            reason: Some(reason),
            last_error,
            done_age_seconds: 0,
        });
        *self.current_job_id.lock().expect("current id mutex") = Some(id);
    }

    pub(crate) fn insert_unclassified_dead_row(&self) {
        self.insert_current_failed_dead_row(QUEUE_NAME, "unknown", Some("legacy failure"));
    }

    pub(crate) fn insert_malformed_requeue_count_and_valid_dead_jobs(&self) {
        // Fill one raw SQL page with rows the adapter rejects before a valid
        // row so this scenario exercises offset pagination, not just overfetch.
        const MALFORMED_ROWS_BEFORE_VALID: usize = 101;
        for _ in 0..MALFORMED_ROWS_BEFORE_VALID {
            self.insert_malformed_requeue_count_dead_job();
        }
        let valid = self.insert_job(JobSeed {
            job_type: QUEUE_NAME,
            status: "Failed",
            attempts: 3,
            max_attempts: Some(3),
            reason: Some("exhausted"),
            last_error: Some("valid row"),
            done_age_seconds: 60,
        });
        *self.newest_job_id.lock().expect("newest job id mutex") = Some(valid);
    }

    pub(crate) fn insert_malformed_requeue_count_dead_job(&self) -> DeadJobId {
        let malformed = self.insert_malformed_requeue_count_dead_job_at(2);
        *self.current_job_id.lock().expect("current id mutex") = Some(malformed.clone());
        malformed
    }

    /// Insert a dead row whose requeue count cannot be parsed.
    ///
    /// The caller chooses the age so a generated listing scenario can place
    /// unrepresentable rows between representable ones.
    pub(super) fn insert_malformed_requeue_count_dead_job_at(
        &self,
        done_age_seconds: i64,
    ) -> DeadJobId {
        let malformed = self.insert_job(JobSeed {
            job_type: QUEUE_NAME,
            status: "Failed",
            attempts: 3,
            max_attempts: Some(3),
            reason: Some("exhausted"),
            last_error: Some("malformed row"),
            done_age_seconds,
        });
        self.set_requeue_count(&malformed, "not-a-number");
        malformed
    }

    pub(crate) fn insert_negative_attempt_dead_row(&self) {
        let id = self.insert_negative_attempt_dead_job_at(0);
        *self.current_job_id.lock().expect("current id mutex") = Some(id);
    }

    /// Insert a dead row whose attempt count is negative.
    ///
    /// The caller chooses the age, for the same reason as
    /// [`Self::insert_malformed_requeue_count_dead_job_at`].
    pub(super) fn insert_negative_attempt_dead_job_at(&self, done_age_seconds: i64) -> DeadJobId {
        self.insert_job(JobSeed {
            job_type: QUEUE_NAME,
            status: "Killed",
            attempts: -1,
            max_attempts: Some(3),
            reason: Some("fatal"),
            last_error: Some("negative attempts"),
            done_age_seconds,
        })
    }

    pub(crate) fn insert_two_dead_jobs(&self) {
        self.insert_job(JobSeed {
            job_type: QUEUE_NAME,
            status: "Failed",
            attempts: 3,
            max_attempts: Some(3),
            reason: Some("exhausted"),
            last_error: None,
            done_age_seconds: 60,
        });
        let newest = self.insert_job(JobSeed {
            job_type: QUEUE_NAME,
            status: "Killed",
            attempts: 1,
            max_attempts: Some(3),
            reason: Some("fatal"),
            last_error: Some("permanent failure"),
            done_age_seconds: 1,
        });
        *self.newest_job_id.lock().expect("newest id mutex") = Some(newest.clone());
        *self.current_job_id.lock().expect("current id mutex") = Some(newest);
    }

    /// Insert a dead row whose stored deadline is an hour in the future.
    ///
    /// The ordinary seed already stores a due deadline, which a requeue that
    /// never rewrote `run_at` would still leave looking eligible. Advancing the
    /// deadline first is what makes the requeue's write observable, so the
    /// reading taken here is kept for the assertion to compare against.
    pub(crate) fn insert_dead_job_with_future_deadline(&self) {
        self.insert_current_failed_dead_row(QUEUE_NAME, "exhausted", Some("future deadline"));
        let id = self.current_id();
        self.set_run_at_after_database_clock(&id);
        let reading = self.read_deadline_and_clock(&id);
        *self
            .deadline_before_requeue
            .lock()
            .expect("pre-requeue deadline mutex") = Some(reading);
    }

    pub(crate) fn insert_unconfigured_dead_job(&self) {
        self.insert_current_failed_dead_row("wildside.unconfigured-queue", "exhausted", None);
    }

    /// Seed a live row and an unrelated dead row beside the current dead target.
    ///
    /// Call this after the scenario has established the target: the rows are
    /// inserted then, so a discard that matched by queue or status rather than
    /// identifier would take them too. The live row's state is read here so
    /// the assertion can compare against what the seed actually stored.
    pub(crate) fn insert_discard_controls(&self) {
        let live_job_id = self.insert_job(JobSeed {
            job_type: QUEUE_NAME,
            status: "Pending",
            attempts: 0,
            max_attempts: Some(3),
            reason: None,
            last_error: None,
            done_age_seconds: 0,
        });
        let live_state_before = self.snapshot(&live_job_id);
        let unrelated_dead_job_id = self.insert_job(JobSeed {
            job_type: QUEUE_NAME,
            status: "Failed",
            attempts: 3,
            max_attempts: Some(3),
            reason: Some("exhausted"),
            last_error: Some("unrelated dead job"),
            done_age_seconds: 60,
        });
        *self
            .discard_controls
            .lock()
            .expect("discard controls mutex") = Some(DiscardControls {
            live_job_id,
            live_state_before,
            unrelated_dead_job_id,
        });
    }
}
