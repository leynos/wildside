//! Attempt-bound acknowledgement for the PostgreSQL queue backend.
//!
//! The acknowledgement shipped with the pinned `apalis-postgres` release
//! matches a delivery on `id` and `lock_by` alone, then overwrites `status`,
//! `attempts`, and `last_result`. Two deliveries can satisfy that predicate at
//! the same time. Orphan recovery returns a stale `Running` row to `Pending`
//! and increments its attempt count, and a worker reusing the same name can
//! claim the row while the original handler is still active; the original
//! acknowledgement then rewrites the replacement's row. Distinct worker names
//! per instance keep another instance's stale delivery away, but they cannot
//! separate two deliveries handled by the same instance.
//!
//! [`AttemptBoundStorage`] composes the same lock and acknowledgement layers as
//! the pinned backend, replacing `PgAck` with [`AttemptBoundAck`]. That
//! acknowledgement adds the attempt count the row holds during the delivery to
//! its predicate. A delivery always reports one more than the count the row
//! stored while it owned the row, so a re-delivered row no longer matches its
//! predecessor's acknowledgement. A worker must be built over this storage to
//! gain the guard; `apalis_postgres::PostgresStorage` alone leaves the narrower
//! predicate in place.

use apalis_core::backend::{Backend, TaskStream};
use apalis_core::error::BoxDynError;
use apalis_core::layers::Stack;
use apalis_core::task::Parts;
use apalis_core::task::status::Status;
use apalis_core::worker::context::WorkerContext;
use apalis_core::worker::ext::ack::{Acknowledge, AcknowledgeLayer};
use apalis_postgres::{Config, LockTaskLayer, PgContext, PgTask, PostgresStorage};
use futures_util::FutureExt;
use futures_util::future::BoxFuture;
use futures_util::stream::BoxStream;
use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;
use tracing::warn;
use ulid::Ulid;

/// Apalis PostgreSQL storage whose acknowledgements are attempt-bound.
///
/// # Examples
///
/// ```text
/// let config = Config::new(queue_name);
/// let storage = AttemptBoundStorage::new(&pool, &config);
/// let worker = WorkerBuilder::new(worker_name)
///     .backend(storage)
///     .build(service);
/// ```
#[derive(Clone)]
pub struct AttemptBoundStorage {
    inner: PostgresStorage<Value>,
    pool: PgPool,
}

impl AttemptBoundStorage {
    /// Wrap PostgreSQL storage for one queue with guarded acknowledgements.
    pub fn new(pool: &PgPool, config: &Config) -> Self {
        Self {
            inner: PostgresStorage::<Value>::new_with_config(pool, config),
            pool: pool.clone(),
        }
    }
}

impl Backend for AttemptBoundStorage {
    type Args = Value;
    type IdType = Ulid;
    type Context = PgContext;
    type Error = sqlx::Error;
    type Stream = TaskStream<PgTask<Value>, sqlx::Error>;
    type Beat = BoxStream<'static, Result<(), sqlx::Error>>;
    type Layer = Stack<LockTaskLayer, AcknowledgeLayer<AttemptBoundAck>>;

    fn heartbeat(&self, worker: &WorkerContext) -> Self::Beat {
        Backend::heartbeat(&self.inner, worker)
    }

    fn middleware(&self) -> Self::Layer {
        Stack::new(
            LockTaskLayer::new(self.pool.clone()),
            AcknowledgeLayer::new(AttemptBoundAck::new(self.pool.clone())),
        )
    }

    fn poll(self, worker: &WorkerContext) -> Self::Stream {
        Backend::poll(self.inner, worker)
    }
}

/// Acknowledge one delivery only while the row still belongs to it.
///
/// The write binds the attempt count the row held during the delivery, so an
/// acknowledgement that arrives after orphan recovery and a re-lock changes no
/// row and reports `RowNotFound`, mirroring what the pinned acknowledgement
/// does when `lock_by` no longer matches.
#[derive(Clone)]
pub struct AttemptBoundAck {
    pool: PgPool,
}

impl AttemptBoundAck {
    /// Build an acknowledgement bound to the attempt count of each delivery.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl<Res: Serialize> Acknowledge<Res, PgContext, Ulid> for AttemptBoundAck {
    type Error = sqlx::Error;
    type Future = BoxFuture<'static, Result<(), Self::Error>>;

    fn ack(
        &mut self,
        res: &Result<Res, BoxDynError>,
        parts: &Parts<PgContext, Ulid>,
    ) -> Self::Future {
        let task_id = parts.task_id;
        let worker_id = parts.ctx.lock_by().clone();
        let attempt = parts.attempt.current();
        let status = acknowledgement_status(parts, res);
        let response = serde_json::to_value(res.as_ref().map_err(|error| error.to_string()));
        let pool = self.pool.clone();

        async move {
            let task_id = task_id
                .ok_or_else(|| sqlx::Error::ColumnNotFound("TASK_ID_FOR_ACK".to_owned()))?
                .to_string();
            let worker_id = worker_id
                .ok_or_else(|| sqlx::Error::ColumnNotFound("WORKER_ID_LOCK_BY".to_owned()))?;
            let (delivered, stored) = attempt_bounds(attempt)?;
            let response = response.map_err(|error| sqlx::Error::Decode(error.into()))?;
            let updated = sqlx::query(
                "UPDATE apalis.jobs
                 SET status = $4, attempts = $2, last_result = $3, done_at = NOW()
                 WHERE id = $1 AND lock_by = $5 AND status = 'Running' AND attempts = $6",
            )
            .bind(&task_id)
            .bind(delivered)
            .bind(&response)
            .bind(status.to_string())
            .bind(&worker_id)
            .bind(stored)
            .execute(&pool)
            .await?;

            if updated.rows_affected() == 0 {
                // A row that moved on is the expected outcome for a stale
                // delivery, not a fault, so the refusal is recorded with the
                // values that explain it and reported as a missing row.
                warn!(
                    task_id = %task_id,
                    worker_id = %worker_id,
                    attempt = delivered,
                    stored_attempt = stored,
                    decision = "acknowledgement_refused",
                    "delivery no longer owns its row; skipping the acknowledgement"
                );
                return Err(sqlx::Error::RowNotFound);
            }
            Ok(())
        }
        .boxed()
    }
}

/// The attempt count a delivery reports and the count its row stored.
///
/// Apalis increments the in-memory attempt on the first poll of a delivery, so
/// while a handler runs the row still holds one less than the delivery counts.
/// Binding the stored count to the acknowledgement is what separates this
/// delivery from a later one that inherited the same row and name.
fn attempt_bounds(attempt: usize) -> Result<(i32, i32), sqlx::Error> {
    let out_of_range = || sqlx::Error::Protocol("delivery attempt out of range".to_owned());
    let delivered = i32::try_from(attempt).map_err(|_| out_of_range())?;
    let stored = delivered.checked_sub(1).ok_or_else(out_of_range)?;
    Ok((delivered, stored))
}

/// Derive the status the pinned acknowledgement would store for this delivery.
///
/// This mirrors `PgAck`'s rule — a success is `Done`, a failure at or past the
/// stored cap is `Killed`, and a failure below it is `Failed` — without its
/// lossy `as` cast, so an out-of-range cap cannot wrap into a spurious `Killed`.
fn acknowledgement_status<Res>(
    parts: &Parts<PgContext, Ulid>,
    res: &Result<Res, BoxDynError>,
) -> Status {
    match res {
        Ok(_) => Status::Done,
        Err(_) if attempt_cap_reached(parts) => Status::Killed,
        Err(_) => Status::Failed,
    }
}

fn attempt_cap_reached(parts: &Parts<PgContext, Ulid>) -> bool {
    let cap = usize::try_from(parts.ctx.max_attempts()).unwrap_or(usize::MAX);
    cap <= parts.attempt.current()
}
