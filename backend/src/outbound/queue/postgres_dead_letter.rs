//! PostgreSQL adapter for the dead-letter queue port.

use std::collections::HashMap;
use std::num::NonZeroU32;

use crate::domain::jobs::retry::JobRetryPolicy;
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};

use crate::domain::ports::{DeadJob, DeadJobId, DeadLetterError, DeadLetterQueue, DeadReason};

const MAX_LAST_ERROR_BYTES: usize = 1024;
const DEAD_PREDICATE: &str = "status = 'Killed' OR (status = 'Failed' AND (max_attempts IS NULL OR attempts >= max_attempts))";
const DEAD_LISTING_SQL: &str = include_str!("sql/dead_listing.sql");

/// Per-queue attempt limits restored when an operator requeues a dead job.
#[derive(Clone, Debug, Default)]
pub struct RequeueBudgets(HashMap<String, NonZeroU32>);

impl RequeueBudgets {
    /// Build queue budgets from the policies used by the enqueue adapters.
    pub fn from_policies<I, S>(policies: I) -> Self
    where
        I: IntoIterator<Item = (S, JobRetryPolicy)>,
        S: Into<String>,
    {
        Self(
            policies
                .into_iter()
                .map(|(queue, policy)| (queue.into(), policy.max_attempts()))
                .collect(),
        )
    }

    fn get(&self, queue: &str) -> Option<NonZeroU32> {
        self.0.get(queue).copied()
    }
}

/// PostgreSQL implementation over Apalis's shared `apalis.jobs` table.
#[derive(Clone)]
pub struct PostgresDeadLetterQueue {
    pool: PgPool,
    requeue_budgets: RequeueBudgets,
}

impl PostgresDeadLetterQueue {
    /// Use the Apalis pool and the policies configured for each queue.
    pub fn new(pool: PgPool, requeue_budgets: RequeueBudgets) -> Self {
        Self {
            pool,
            requeue_budgets,
        }
    }
}

#[derive(FromRow)]
struct DeadJobRow {
    id: String,
    job_type: String,
    attempts: i32,
    dead_reason: Option<String>,
    requeue_count: Option<String>,
    last_error: Option<String>,
    dead_at: Option<DateTime<Utc>>,
}

#[derive(FromRow)]
struct LockedDeadJob {
    job_type: String,
    attempts: i32,
    requeue_count: Option<String>,
}

#[async_trait::async_trait]
impl DeadLetterQueue for PostgresDeadLetterQueue {
    async fn list_dead(&self, limit: u32) -> Result<Vec<DeadJob>, DeadLetterError> {
        let rows = sqlx::query_as::<_, DeadJobRow>(DEAD_LISTING_SQL)
            .bind(i64::from(limit))
            .fetch_all(&self.pool)
            .await
            .map_err(|error| unavailable("list dead jobs", error))?;

        Ok(rows.into_iter().filter_map(dead_job_from_row).collect())
    }

    async fn requeue(&self, id: &DeadJobId) -> Result<(), DeadLetterError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| unavailable("begin dead-job requeue", error))?;
        let select = format!(
            "SELECT job_type, attempts,
                    metadata->>'wildside.retry.requeue_count' AS requeue_count
             FROM apalis.jobs
             WHERE id = $1 AND ({DEAD_PREDICATE})
             FOR UPDATE"
        );
        let dead_job = sqlx::query_as::<_, LockedDeadJob>(&select)
            .bind(id.as_str())
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|error| unavailable("lock dead job for requeue", error))?
            .ok_or_else(|| DeadLetterError::not_dead(id.as_str()))?;
        if dead_job.attempts < 0 {
            return Err(DeadLetterError::invalid_job_data(
                "attempt count is negative",
            ));
        }
        let budget = self
            .requeue_budgets
            .get(&dead_job.job_type)
            .ok_or_else(|| DeadLetterError::unknown_queue(&dead_job.job_type))?;
        let max_attempts = i32::try_from(budget.get()).map_err(|_| {
            DeadLetterError::unavailable(format!(
                "retry budget for queue {} exceeds Apalis integer range",
                dead_job.job_type
            ))
        })?;
        let next_count = next_requeue_count(dead_job.requeue_count.as_deref())
            .map_err(|()| DeadLetterError::invalid_job_data("requeue count is not numeric"))?;
        let update = format!(
            "UPDATE apalis.jobs
             SET status = 'Pending', attempts = 0, max_attempts = $2,
                 run_at = now(), lock_by = NULL, lock_at = NULL,
                 metadata = jsonb_set(
                     COALESCE(metadata, '{{}}'::jsonb) - 'wildside.retry.dead_reason',
                     '{{wildside.retry.requeue_count}}', to_jsonb($3::bigint), true
                 )
             WHERE id = $1 AND job_type = $4 AND ({DEAD_PREDICATE})"
        );
        let result = sqlx::query(&update)
            .bind(id.as_str())
            .bind(max_attempts)
            .bind(i64::from(next_count))
            .bind(&dead_job.job_type)
            .execute(&mut *transaction)
            .await
            .map_err(|error| unavailable("requeue dead job", error))?;
        if result.rows_affected() != 1 {
            return Err(DeadLetterError::not_dead(id.as_str()));
        }
        transaction
            .commit()
            .await
            .map_err(|error| unavailable("commit dead-job requeue", error))
    }

    async fn discard(&self, id: &DeadJobId) -> Result<(), DeadLetterError> {
        let query = format!(
            "DELETE FROM apalis.jobs
             WHERE id = $1 AND ({DEAD_PREDICATE})"
        );
        let result = sqlx::query(&query)
            .bind(id.as_str())
            .execute(&self.pool)
            .await
            .map_err(|error| unavailable("discard dead job", error))?;
        if result.rows_affected() != 1 {
            return Err(DeadLetterError::not_dead(id.as_str()));
        }
        tracing::info!(
            job_id = %id.as_str(),
            "discarded dead-lettered job"
        );
        Ok(())
    }
}

fn dead_job_from_row(row: DeadJobRow) -> Option<DeadJob> {
    let attempts = match u32::try_from(row.attempts) {
        Ok(attempts) => attempts,
        Err(_) => {
            tracing::warn!(
                job_id = %row.id,
                "omitting dead-letter row with a negative attempt count"
            );
            return None;
        }
    };
    let requeue_count = match next_count_value(row.requeue_count.as_deref()) {
        Ok(count) => count,
        Err(()) => {
            tracing::warn!(
                job_id = %row.id,
                "omitting dead-letter row with invalid requeue-count metadata"
            );
            return None;
        }
    };
    let last_error = row
        .last_error
        .map(|error| truncate_utf8(&error, MAX_LAST_ERROR_BYTES));
    Some(DeadJob {
        id: DeadJobId::new(row.id),
        job_type: row.job_type,
        reason: dead_reason_from_metadata(row.dead_reason.as_deref()),
        attempts,
        last_error,
        requeue_count,
        dead_at: row.dead_at,
    })
}

fn dead_reason_from_metadata(value: Option<&str>) -> DeadReason {
    match value {
        Some("fatal") => DeadReason::Fatal,
        Some("exhausted") => DeadReason::Exhausted,
        _ => DeadReason::Unclassified,
    }
}

#[cfg(test)]
fn is_dead_row(status: &str, attempts: i32, max_attempts: Option<i32>) -> bool {
    status == "Killed" || (status == "Failed" && max_attempts.is_none_or(|limit| attempts >= limit))
}

fn next_requeue_count(value: Option<&str>) -> Result<u32, ()> {
    let current = next_count_value(value)?;
    Ok(current.saturating_add(1))
}

fn next_count_value(value: Option<&str>) -> Result<u32, ()> {
    value
        .map(|value| {
            value
                .parse::<u64>()
                .map(|count| count.min(u64::from(u32::MAX)) as u32)
                .map_err(|_| ())
        })
        .unwrap_or(Ok(0))
}

fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}

fn unavailable(operation: &str, error: impl std::fmt::Display) -> DeadLetterError {
    DeadLetterError::unavailable(format!("{operation}: {error}"))
}

#[cfg(test)]
mod tests;
