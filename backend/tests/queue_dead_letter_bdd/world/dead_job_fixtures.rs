//! Scenario seeds for dead, legacy, and malformed Apalis rows.

use backend::domain::ports::DeadJobId;

use super::rows::JobSeed;
use super::{QUEUE_NAME, QueueDeadLetterWorld};

impl QueueDeadLetterWorld {
    pub fn insert_unclassified_dead_row(&self) {
        let id = self.insert_job(JobSeed {
            job_type: QUEUE_NAME,
            status: "Failed",
            attempts: 3,
            max_attempts: Some(3),
            reason: Some("unknown"),
            last_error: Some("legacy failure"),
            done_age_seconds: 0,
        });
        *self.current_job_id.lock().expect("current id mutex") = Some(id);
    }

    pub fn insert_malformed_requeue_count_and_valid_dead_jobs(&self) {
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

    pub fn insert_malformed_requeue_count_dead_job(&self) -> DeadJobId {
        let malformed = self.insert_job(JobSeed {
            job_type: QUEUE_NAME,
            status: "Failed",
            attempts: 3,
            max_attempts: Some(3),
            reason: Some("exhausted"),
            last_error: Some("malformed row"),
            done_age_seconds: 2,
        });
        self.set_requeue_count(&malformed, "not-a-number");
        *self.current_job_id.lock().expect("current id mutex") = Some(malformed.clone());
        malformed
    }

    pub fn insert_negative_attempt_dead_row(&self) {
        let id = self.insert_job(JobSeed {
            job_type: QUEUE_NAME,
            status: "Killed",
            attempts: -1,
            max_attempts: Some(3),
            reason: Some("fatal"),
            last_error: Some("negative attempts"),
            done_age_seconds: 0,
        });
        *self.current_job_id.lock().expect("current id mutex") = Some(id);
    }

    pub fn insert_two_dead_jobs(&self) {
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

    pub fn insert_unconfigured_dead_job(&self) {
        let id = self.insert_job(JobSeed {
            job_type: "wildside.unconfigured-queue",
            status: "Failed",
            attempts: 3,
            max_attempts: Some(3),
            reason: Some("exhausted"),
            last_error: None,
            done_age_seconds: 0,
        });
        *self.current_job_id.lock().expect("current id mutex") = Some(id);
    }
}
