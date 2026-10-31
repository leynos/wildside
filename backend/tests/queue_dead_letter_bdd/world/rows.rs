//! Database row creation and snapshots for dead-letter scenarios.

use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sqlx::FromRow;
use uuid::Uuid;

use backend::domain::ports::DeadJobId;

use super::{QUEUE_NAME, QueueDeadLetterWorld};

const CROCKFORD_BASE32: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

pub(super) struct JobSeed<'a> {
    pub(super) job_type: &'a str,
    pub(super) status: &'a str,
    pub(super) attempts: i32,
    pub(super) max_attempts: Option<i32>,
    pub(super) reason: Option<&'a str>,
    pub(super) last_error: Option<&'a str>,
    pub(super) done_age_seconds: i64,
}

#[derive(Clone, Debug, FromRow, PartialEq)]
pub(super) struct JobSnapshot {
    id: String,
    job_type: String,
    pub(super) status: String,
    pub(super) attempts: i32,
    pub(super) max_attempts: Option<i32>,
    pub(super) run_at: DateTime<Utc>,
    pub(super) lock_by: Option<String>,
    pub(super) lock_at: Option<DateTime<Utc>>,
    done_at: Option<DateTime<Utc>>,
    pub(super) metadata: Option<Value>,
    last_result: Option<Value>,
}

impl QueueDeadLetterWorld {
    pub(super) fn remember_probe_job_id(&self) {
        let id: String = self
            .runtime
            .block_on(
                sqlx::query_scalar(
                    "SELECT id FROM apalis.jobs
                     WHERE job_type = $1
                       AND convert_from(job, 'UTF8')::jsonb->>'probeId' = $2",
                )
                .bind(QUEUE_NAME)
                .bind(&self.probe_id)
                .fetch_one(&self.pool),
            )
            .expect("read enqueued probe id");
        *self.current_job_id.lock().expect("current id mutex") = Some(DeadJobId::new(id));
    }

    pub(super) fn insert_job(&self, seed: JobSeed<'_>) -> DeadJobId {
        let id = apalis_job_id();
        let metadata = seed
            .reason
            .map(|reason| json!({"wildside.retry.dead_reason": reason}));
        let last_result = seed.last_error.map(|message| json!({"Err": message}));
        self.runtime
            .block_on(
                sqlx::query(
                    "INSERT INTO apalis.jobs
                        (id, job_type, job, status, attempts, max_attempts, run_at,
                         last_result, done_at, metadata)
                     VALUES ($1, $2, convert_to($3, 'UTF8'), $4, $5, $6, now(),
                             $7, now() - ($8::bigint * interval '1 second'), $9)",
                )
                .bind(&id)
                .bind(seed.job_type)
                .bind(json!({"v": "v1", "probeId": self.probe_id}).to_string())
                .bind(seed.status)
                .bind(seed.attempts)
                .bind(seed.max_attempts)
                .bind(last_result)
                .bind(seed.done_age_seconds)
                .bind(metadata)
                .execute(&self.pool),
            )
            .expect("seed Apalis job row");
        DeadJobId::new(id)
    }

    pub(super) fn current_id(&self) -> DeadJobId {
        self.current_job_id
            .lock()
            .expect("current id mutex")
            .clone()
            .expect("current job id")
    }

    pub(super) fn snapshot(&self, id: &DeadJobId) -> JobSnapshot {
        self.runtime
            .block_on(
                sqlx::query_as::<_, JobSnapshot>(
                    "SELECT id, job_type, status, attempts, max_attempts, run_at,
                            lock_by, lock_at, done_at, metadata, last_result
                     FROM apalis.jobs WHERE id = $1",
                )
                .bind(id.as_str())
                .fetch_one(&self.pool),
            )
            .expect("read Apalis job snapshot")
    }

    pub(super) fn set_requeue_count(&self, id: &DeadJobId, value: &str) {
        self.runtime
            .block_on(
                sqlx::query(
                    "UPDATE apalis.jobs
                     SET metadata = jsonb_set(
                         COALESCE(metadata, '{}'::jsonb),
                         '{wildside.retry.requeue_count}', to_jsonb($2::text), true
                     )
                     WHERE id = $1",
                )
                .bind(id.as_str())
                .bind(value)
                .execute(&self.pool),
            )
            .expect("set requeue-count test metadata");
    }
}

/// Generates a unique ULID-shaped identifier using the existing UUID crate.
///
/// Apalis parses manually seeded job IDs as ULIDs. The fixtures need their
/// syntax and uniqueness, but do not depend on ULID time ordering.
fn apalis_job_id() -> String {
    let mut value = Uuid::new_v4().as_u128();
    let mut encoded = [b'0'; 26];

    for digit in encoded.iter_mut().rev() {
        *digit = CROCKFORD_BASE32[(value & 0b1_1111) as usize];
        value >>= 5;
    }

    String::from_utf8(encoded.to_vec()).expect("Crockford Base32 contains valid UTF-8")
}

#[cfg(test)]
mod tests {
    //! Verify fixture identifiers retain the ULID syntax Apalis expects.

    use super::{CROCKFORD_BASE32, apalis_job_id};

    #[test]
    fn generated_apalis_job_ids_have_ulid_syntax() {
        let id = apalis_job_id();

        assert_eq!(id.len(), 26);
        assert!(id.as_bytes()[0] <= b'7');
        assert!(id.bytes().all(|digit| CROCKFORD_BASE32.contains(&digit)));
    }
}
