//! Generation-bound locking for the PostgreSQL queue backend.
//!
//! The attempt count is not a delivery generation. Orphan recovery increments
//! it, but a dead-letter requeue resets it to zero, so an acknowledgement from
//! a delivery that lost the row can match the identifier, worker name,
//! `Running` status, and stored attempt count of a later delivery of the same
//! row.
//!
//! This layer therefore writes a claim token into the row's metadata every time
//! a delivery is locked, and carries the same token in the task's context. The
//! acknowledgement requires the row to still hold the token its delivery
//! claimed under. Recovery, retry scheduling, terminal writes, and the cap
//! clamp update other columns without touching unrelated metadata keys, and a
//! dead-letter requeue preserves them, so only a new claim changes the token.
//!
//! The claim itself is bound to the delivery that fetched the row. A fetched
//! task may wait in the worker's buffer while its row moves on: orphan recovery
//! returns the row to `Pending`, clears `lock_by`, and increments `attempts`,
//! and a dead-letter requeue clears `lock_by` and resets `attempts` to zero.
//! Either way another worker can fetch the row in the meantime. The claim
//! therefore requires the row to still name this worker in `lock_by` and to
//! still hold the attempt count this delivery was built with, which refuses an
//! obsolete delivery before its handler is polled. Both predicates hold for a
//! fresh delivery, because `apalis.get_jobs` recorded this worker in `lock_by`
//! during the fetch and nothing else changes the count before the claim.

use apalis_core::error::{AbortError, BoxDynError};
use apalis_core::layers::{Layer, Service};
use apalis_core::worker::context::WorkerContext;
use apalis_postgres::{PgContext, PgPool, PgTask};
use futures_util::FutureExt;
use futures_util::future::BoxFuture;
use serde_json::Value;
use tracing::warn;
use uuid::Uuid;

/// Metadata key that names the delivery generation that owns the row.
pub(super) const CLAIM_TOKEN_KEY: &str = "wildside.retry.claim_token";

/// Read the claim token a task's context carries, if it has one.
pub(super) fn claim_token(ctx: &PgContext) -> Option<&str> {
    ctx.meta().get(CLAIM_TOKEN_KEY).and_then(Value::as_str)
}

/// Middleware that claims a row for one delivery generation.
///
/// The lock step is the same as the pinned backend's: a `Pending`, `Queued`, or
/// retryable `Failed` row that is due becomes `Running` under the claiming
/// worker. It additionally stores a fresh token, which makes the claim
/// observable to the acknowledgement.
#[derive(Clone, Debug)]
pub struct AttemptBoundLock {
    pool: PgPool,
}

impl AttemptBoundLock {
    /// Build the lock layer for one PostgreSQL pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl<S> Layer<S> for AttemptBoundLock {
    type Service = AttemptBoundLockService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AttemptBoundLockService {
            inner,
            pool: self.pool.clone(),
        }
    }
}

/// The service the lock layer wraps, holding the pool it claims against.
#[derive(Clone, Debug)]
pub struct AttemptBoundLockService<S> {
    inner: S,
    pool: PgPool,
}

impl<S, Args> Service<PgTask<Args>> for AttemptBoundLockService<S>
where
    S: Service<PgTask<Args>> + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<BoxDynError>,
    Args: Send + 'static,
{
    type Response = S::Response;
    type Error = BoxDynError;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, mut req: PgTask<Args>) -> Self::Future {
        let pool = self.pool.clone();
        let (worker_id, task_id, fetched_attempt) = match claim_inputs(&req) {
            Ok(inputs) => inputs,
            Err(error) => return async { Err(AbortError::new(error).into()) }.boxed(),
        };
        // A version-4 UUID is random rather than time-ordered, which is all a
        // claim token needs: it must never repeat, and nothing orders by it.
        let token = Uuid::new_v4().to_string();
        req.parts.ctx = with_claim_token(req.parts.ctx, &token);
        let fut = self.inner.call(req);
        async move {
            let claim = Claim {
                task_id: &task_id,
                worker: &worker_id,
                fetched_attempt,
                token: &token,
            };
            claim_row(&pool, &claim).await.map_err(AbortError::new)?;
            fut.await.map_err(Into::into)
        }
        .boxed()
    }
}

/// Read the delivery fields the claim compares against the row.
///
/// A delivery always arrives from a worker that fetched it, so a missing worker
/// context, a missing task identifier, or an attempt count that cannot be
/// stored as the row's `INTEGER` is a wiring fault rather than a row that
/// merely moved on.
fn claim_inputs<Args>(req: &PgTask<Args>) -> Result<(String, String, i32), sqlx::Error> {
    let worker_id = req
        .parts
        .data
        .get::<WorkerContext>()
        .map(|worker| worker.name().to_owned())
        .ok_or_else(|| sqlx::Error::ColumnNotFound("WORKER_CONTEXT_FOR_LOCK".to_owned()))?;
    let task_id = req
        .parts
        .task_id
        .map(|id| id.inner().to_string())
        .ok_or_else(|| sqlx::Error::ColumnNotFound("TASK_ID_FOR_LOCK".to_owned()))?;
    // The lock layer sits outside the worker's attempt tracker, so the counter
    // still holds the value the fetched row carried; it is incremented only
    // when the handler's future is first polled, which happens after the claim.
    let fetched_attempt = i32::try_from(req.parts.attempt.current())
        .map_err(|_| sqlx::Error::Protocol("FETCHED_ATTEMPT_FOR_LOCK".to_owned()))?;
    Ok((worker_id, task_id, fetched_attempt))
}

/// Copy a context with the claim token recorded in its metadata map.
fn with_claim_token(ctx: PgContext, token: &str) -> PgContext {
    let mut meta = ctx.meta().clone();
    meta.insert(CLAIM_TOKEN_KEY.to_owned(), Value::String(token.to_owned()));
    ctx.with_meta(meta)
}

/// The fields one delivery contributes to the claim that binds it to its row.
struct Claim<'a> {
    task_id: &'a str,
    worker: &'a str,
    fetched_attempt: i32,
    token: &'a str,
}

/// Claim one due row for a delivery and stamp it with the claim token.
///
/// The predicate mirrors `lock_by_id.sql` from the pinned release and adds the
/// two checks that tie the claim to the fetch this delivery came from. The row
/// must still name this worker in `lock_by`, which `apalis.get_jobs` recorded
/// and orphan recovery or a requeue clears, and it must still hold the attempt
/// count the delivery was built with, which both of those steps change. A row
/// that no longer satisfies the predicate is reported as missing, which is what
/// a delivery that lost the row should see before its handler runs.
async fn claim_row(pool: &PgPool, claim: &Claim<'_>) -> Result<(), sqlx::Error> {
    let ids = [claim.task_id.to_owned()];
    let updated = sqlx::query(
        "UPDATE apalis.jobs
         SET status = 'Running', lock_at = now(), lock_by = $2,
             metadata = jsonb_set(
                 COALESCE(metadata, '{}'::jsonb),
                 '{wildside.retry.claim_token}',
                 to_jsonb($3::text),
                 true
             )
         WHERE (status = 'Pending'
                OR status = 'Queued'
                OR (status = 'Failed' AND attempts < max_attempts))
           AND run_at < now()
           AND id = ANY($1)
           AND lock_by = $2
           AND attempts = $4",
    )
    .bind(&ids[..])
    .bind(claim.worker)
    .bind(claim.token)
    .bind(claim.fetched_attempt)
    .execute(pool)
    .await?;

    if updated.rows_affected() == 1 {
        return Ok(());
    }
    // Only a delivery that fetched the row reaches this point, so a refused
    // claim means the row moved on between the fetch and the handler. Report
    // the delivery's own fields: the row no longer holds them, and the caller
    // only sees `RowNotFound`.
    warn!(
        decision = "claim_refused",
        task_id = claim.task_id,
        attempt = claim.fetched_attempt,
        "claim update affected no row; the delivery no longer owns the row"
    );
    Err(sqlx::Error::RowNotFound)
}
