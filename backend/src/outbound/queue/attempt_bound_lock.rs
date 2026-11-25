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
//! acknowledgement and the wrapper's retry, terminal, and cap-clamp writes all
//! require the row to still hold the token its delivery claimed under.
//! Recovery and a dead-letter requeue update other columns without touching
//! unrelated metadata keys, and the guarded writes preserve them, so only a new
//! claim changes the token.
//!
//! The claim itself is bound to the delivery that fetched the row. A fetched
//! task may wait in the worker's buffer while its row moves on: orphan recovery
//! returns the row to `Pending`, clears `lock_by`, and increments `attempts`;
//! an ordinary retry acknowledgement increments `attempts`; and a dead-letter
//! requeue clears `lock_by` and resets `attempts` to zero. Another worker can
//! fetch the row in each case. The claim therefore requires the row to still
//! name this worker in `lock_by`, to still hold the attempt count this delivery
//! was built with, and to still report the dead-letter requeue generation the
//! fetch captured, which refuses an obsolete delivery before its handler is
//! polled. All three hold for a fresh delivery, because `apalis.get_jobs`
//! records this worker in `lock_by` during the fetch and nothing else changes
//! the row before the claim. A requeue is the one path that resets the count,
//! and it increments `wildside.retry.requeue_count` as it does so, so that
//! counter separates a delivery fetched before the requeue from the replacement
//! fetched after it. `lock_at` cannot serve that purpose: the pinned context
//! exposes it at whole-second precision, so two fetches within one second are
//! indistinguishable.

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

/// Metadata key that counts the dead-letter requeues a row has undergone.
const REQUEUE_COUNT_KEY: &str = "wildside.retry.requeue_count";

/// The requeue generation of a row that has never been requeued.
///
/// The claim's SQL `COALESCE` reads a missing or JSON-null counter as the same
/// text, so these two defaults must stay equal.
const DEFAULT_REQUEUE_GENERATION: &str = "0";

/// Read the claim token a task's context carries, if it has one.
pub(super) fn claim_token(ctx: &PgContext) -> Option<&str> {
    ctx.meta().get(CLAIM_TOKEN_KEY).and_then(Value::as_str)
}

/// Read the dead-letter requeue generation a fetched delivery carries.
///
/// The dead-letter adapter stores the counter as a `jsonb` number, which
/// PostgreSQL's `->>` renders as decimal text. A row written by another tool
/// may hold a JSON string instead, so both shapes are normalized to the text
/// the claim's predicate compares. A missing or JSON-null counter, and any
/// other shape, reads as [`DEFAULT_REQUEUE_GENERATION`]; the latter makes the
/// claim refuse a row whose counter this delivery cannot identify exactly.
fn fetched_requeue_generation(ctx: &PgContext) -> String {
    match ctx.meta().get(REQUEUE_COUNT_KEY) {
        Some(Value::Number(count)) => count.to_string(),
        Some(Value::String(text)) => text.clone(),
        _ => DEFAULT_REQUEUE_GENERATION.to_owned(),
    }
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
        // The generation must be read before the claim token is written into
        // the context, because that write replaces the metadata map.
        let requeue_generation = fetched_requeue_generation(&req.parts.ctx);
        // A version-4 UUID is random rather than time-ordered, which is all a
        // claim token needs: it must never repeat, and nothing orders by it.
        let token = Uuid::new_v4().to_string();
        // The refusal below reports the queue, and the context that carries it
        // moves into the inner service with `req`.
        let queue = req.parts.ctx.queue().clone();
        req.parts.ctx = with_claim_token(req.parts.ctx, &token);
        let fut = self.inner.call(req);
        async move {
            let claim = Claim {
                task_id: &task_id,
                worker: &worker_id,
                queue: queue.as_deref(),
                fetched_attempt,
                requeue_generation: &requeue_generation,
                token: &token,
            };
            claim_row(&pool, &claim).await.map_err(AbortError::new)?;
            fut.await.map_err(Into::into)
        }
        .boxed()
    }
}

/// Read the delivery identity the claim compares against the row.
///
/// A delivery always arrives from a worker that fetched it, so a missing worker
/// context, a missing task identifier, or an attempt count that cannot be
/// stored as the row's `INTEGER` is a wiring fault rather than a row that
/// merely moved on. The requeue generation is read separately, because it is
/// not an identity the delivery must carry but a value the fetch captured.
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
    queue: Option<&'a str>,
    fetched_attempt: i32,
    requeue_generation: &'a str,
    token: &'a str,
}

/// Claim one due row for a delivery and stamp it with the claim token.
///
/// The predicate mirrors `lock_by_id.sql` from the pinned release and adds the
/// three checks that tie the claim to the fetch this delivery came from. The row
/// must still name this worker in `lock_by`, still hold the attempt count the
/// delivery was built with, and still report the requeue generation the fetch
/// captured. Between them, these checks refuse every way a row can move on
/// after its delivery was fetched:
///
/// - orphan recovery returns the row to `Pending`, clears `lock_by`, and
///   increments `attempts`;
/// - an ordinary retry acknowledgement increments `attempts`;
/// - a dead-letter requeue clears `lock_by` and resets `attempts` to zero, so
///   once the same worker fetches the replacement, only the requeue counter
///   still separates the generations.
///
/// A row that no longer satisfies the predicate is reported as missing, which
/// is what a delivery that lost the row should see before its handler runs.
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
           AND attempts = $4
           AND COALESCE(metadata->>'wildside.retry.requeue_count', '0') = $5",
    )
    .bind(&ids[..])
    .bind(claim.worker)
    .bind(claim.token)
    .bind(claim.fetched_attempt)
    .bind(claim.requeue_generation)
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
        queue = claim.queue,
        task_id = claim.task_id,
        attempt = claim.fetched_attempt,
        requeue_generation = claim.requeue_generation,
        "claim update affected no row; the delivery no longer owns the row"
    );
    Err(sqlx::Error::RowNotFound)
}

#[cfg(test)]
mod tests {
    //! Tests for the requeue generation the claim compares.

    use apalis_postgres::PgContext;
    use rstest::rstest;
    use serde_json::{Map, Value, json};

    use super::{DEFAULT_REQUEUE_GENERATION, REQUEUE_COUNT_KEY, fetched_requeue_generation};

    /// Build a delivery context whose metadata is `stored`.
    fn context(stored: Option<Value>) -> PgContext {
        let mut meta = Map::new();
        if let Some(value) = stored {
            meta.insert(REQUEUE_COUNT_KEY.to_owned(), value);
        }
        PgContext::new().with_meta(meta)
    }

    /// Scenario: the stored counter has each shape a fetch can return.
    ///
    /// Invariant: a `jsonb` number and a JSON string normalize to the same
    /// decimal text PostgreSQL's `->>` renders, and a row that never carried a
    /// counter reads as generation zero. A shape this delivery cannot identify
    /// exactly falls back to zero, so the claim refuses the row rather than
    /// matching a generation it did not read.
    #[rstest]
    #[case::absent(None, "0")]
    #[case::json_null(Some(Value::Null), "0")]
    #[case::zero(Some(json!(0)), "0")]
    #[case::number(Some(json!(3)), "3")]
    #[case::large_number(Some(json!(u64::from(u32::MAX))), "4294967295")]
    #[case::string(Some(json!("7")), "7")]
    #[case::unreadable_shape(Some(json!({"nested": 1})), "0")]
    fn a_fetched_counter_normalizes_to_the_text_the_claim_compares(
        #[case] stored: Option<Value>,
        #[case] expected: &str,
    ) {
        let ctx = context(stored);

        assert_eq!(
            fetched_requeue_generation(&ctx),
            expected,
            "a stored counter must normalize to the text the row predicate uses"
        );
    }

    /// The Rust default and the SQL `COALESCE` literal must not drift apart.
    #[test]
    fn the_default_generation_matches_the_claim_default() {
        assert_eq!(
            DEFAULT_REQUEUE_GENERATION, "0",
            "the Rust default must match the SQL claim's COALESCE literal"
        );
    }
}
