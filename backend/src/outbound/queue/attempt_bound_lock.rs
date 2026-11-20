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

use apalis_core::error::{AbortError, BoxDynError};
use apalis_core::layers::{Layer, Service};
use apalis_core::worker::context::WorkerContext;
use apalis_postgres::{PgContext, PgPool, PgTask};
use futures_util::FutureExt;
use futures_util::future::BoxFuture;
use serde_json::Value;
use ulid::Ulid;

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
        // A delivery always arrives from a worker, so a context without one is
        // a wiring fault rather than a row that merely moved on.
        let worker_id = match req.parts.data.get::<WorkerContext>() {
            Some(worker) => worker.name().to_owned(),
            None => {
                return async {
                    Err(sqlx::Error::ColumnNotFound("WORKER_CONTEXT_FOR_LOCK".to_owned()).into())
                }
                .boxed();
            }
        };
        let task_id = match req.parts.task_id {
            Some(id) => id.inner().to_string(),
            None => {
                return async {
                    Err(sqlx::Error::ColumnNotFound("TASK_ID_FOR_LOCK".to_owned()).into())
                }
                .boxed();
            }
        };
        let token = Ulid::new().to_string();
        req.parts.ctx = with_claim_token(req.parts.ctx, &token);
        let fut = self.inner.call(req);
        async move {
            claim(&pool, &task_id, &worker_id, &token)
                .await
                .map_err(AbortError::new)?;
            fut.await.map_err(Into::into)
        }
        .boxed()
    }
}

/// Copy a context with the claim token recorded in its metadata map.
fn with_claim_token(ctx: PgContext, token: &str) -> PgContext {
    let mut meta = ctx.meta().clone();
    meta.insert(CLAIM_TOKEN_KEY.to_owned(), Value::String(token.to_owned()));
    ctx.with_meta(meta)
}

/// Claim one due row for a delivery and stamp it with the claim token.
///
/// The predicate mirrors `lock_by_id.sql` from the pinned release. A row that
/// no longer satisfies it is reported as missing, which is what a delivery
/// racing another claim should see.
async fn claim(
    pool: &PgPool,
    task_id: &str,
    worker_id: &str,
    token: &str,
) -> Result<(), sqlx::Error> {
    let ids = [task_id.to_owned()];
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
           AND id = ANY($1)",
    )
    .bind(&ids[..])
    .bind(worker_id)
    .bind(token)
    .execute(pool)
    .await?;

    if updated.rows_affected() == 1 {
        return Ok(());
    }
    Err(sqlx::Error::RowNotFound)
}
