//! Delivery fixtures for the retry wrapper's unit tests.
//!
//! Each builder produces a `PgTask` that carries one of the identity
//! combinations a worker can hand the wrapper. The claimed-delivery default
//! names [`TASK_ID`], records [`WORKER`] in both places the wrapper compares,
//! and carries [`CLAIM_TOKEN`] in its metadata, which is the state a delivery
//! reaches the wrapper in after the lock step. Builders remove one part at a
//! time so a test can pin the refusal each missing part causes.

use apalis_core::task::Task;
use apalis_core::task::attempt::Attempt;
use apalis_core::task::extensions::Extensions;
use apalis_core::worker::context::WorkerContext;
use apalis_postgres::{PgContext, PgTask};
use serde_json::Value;

use super::doubles::{CLAIM_TOKEN, TASK_ID, WORKER};

/// Build a delivery whose worker context and `lock_by` agree.
pub(super) fn task(payload: Value, attempt: usize) -> PgTask<Value> {
    task_with(
        payload,
        attempt,
        DeliveryIdentity::new(Some(WORKER), Some(WORKER)),
    )
}

/// The identity a task double carries.
///
/// The wrapper writes only when it can name both the row and the worker that
/// owns it, so tests vary these fields independently. The identity starts out
/// naming [`TASK_ID`] and carrying [`CLAIM_TOKEN`];
/// [`without_task_id`](Self::without_task_id) and
/// [`without_claim_token`](Self::without_claim_token) remove those values to
/// exercise the refusal paths.
#[derive(Clone, Copy, Debug)]
pub(super) struct DeliveryIdentity<'a> {
    worker: Option<&'a str>,
    lock_by: Option<&'a str>,
    task_id: Option<&'a str>,
    claim_token: Option<&'a str>,
}

impl<'a> DeliveryIdentity<'a> {
    /// An identity for a claimed delivery that names [`TASK_ID`].
    ///
    /// `worker` is the name stored in the task's [`WorkerContext`]; `lock_by`
    /// is the worker recorded on the Apalis context. The wrapper persists a
    /// failure only when both are present and agree, so tests pass each
    /// missing or mismatched combination.
    pub(super) fn new(worker: Option<&'a str>, lock_by: Option<&'a str>) -> Self {
        Self {
            worker,
            lock_by,
            task_id: Some(TASK_ID),
            claim_token: Some(CLAIM_TOKEN),
        }
    }

    /// The same identity on a delivery that carries no task identifier.
    ///
    /// The wrapper needs the identifier to address the row it would update, so
    /// a delivery without one must be refused even when the worker identities
    /// agree.
    pub(super) fn without_task_id(mut self) -> Self {
        self.task_id = None;
        self
    }

    /// The same identity on a delivery that carries no claim token.
    ///
    /// A delivery built without the lock layer has no token, and its guarded
    /// writes bind the empty string, which matches no row.
    pub(super) fn without_claim_token(mut self) -> Self {
        self.claim_token = None;
        self
    }
}

/// Build a delivery with the chosen identity and a five-attempt stored cap.
pub(super) fn task_with(
    payload: Value,
    attempt: usize,
    identity: DeliveryIdentity<'_>,
) -> PgTask<Value> {
    task_with_budget(payload, attempt, 5, identity)
}

/// Build a delivery whose context carries an explicit stored attempt cap.
///
/// The wrapper compares the delivered attempt with the cap Apalis restored
/// from the row, so a test that exercises a spent cap has to set both numbers
/// the way a recovered row would carry them.
pub(super) fn task_with_budget(
    payload: Value,
    attempt: usize,
    max_attempts: i32,
    identity: DeliveryIdentity<'_>,
) -> PgTask<Value> {
    let mut data = Extensions::default();
    if let Some(worker) = identity.worker {
        // `WorkerContext::new` carries the service type only for diagnostics,
        // so the unit is sufficient here.
        data.insert(WorkerContext::new::<()>(worker));
    }
    let mut metadata = serde_json::Map::new();
    if let Some(token) = identity.claim_token {
        metadata.insert(
            crate::outbound::queue::attempt_bound_lock::CLAIM_TOKEN_KEY.to_owned(),
            Value::String(token.to_owned()),
        );
    }
    let mut builder = Task::builder(payload)
        .with_ctx(
            PgContext::new()
                .with_max_attempts(max_attempts)
                .with_lock_by(identity.lock_by.map(str::to_owned))
                .with_meta(metadata),
        )
        .with_data(data)
        .with_attempt(Attempt::new_with_value(attempt));
    if let Some(task_id) = identity.task_id {
        builder = builder.with_task_id(task_id.parse().expect("task id"));
    }
    builder.build()
}
