//! Shared PostgreSQL fixtures for the Apalis retry canaries.
//!
//! Every canary pins one behaviour of `apalis-postgres` rc.8 against a real
//! database. This module owns the environment, the row builders, and the two
//! fetch helpers, so each canary module reads as an assertion about the
//! contract rather than as database setup.
//!
//! The two fetch helpers answer different questions. [`fetch_one_task`] drives
//! the real backend stream and is the only way to see what a delivery carries,
//! but a window that expires cannot distinguish an empty poll from one that
//! never completed. [`claim_due_rows`] calls `apalis.get_jobs` directly, so it
//! completes or errors, and a count of zero is evidence about the predicate.
//!
//! The embedded cluster and its worker registration live in [`environment`],
//! the row and delivery builders live in [`rows`], and the identifier
//! newtypes live in [`ids`]. Items carry `pub(crate)` visibility rather than
//! `pub(super)` because a fixture's fields and methods are used from sibling
//! canary modules, not only from this one.

// The parent module is loaded through `#[path]`, which stops Rust from
// deriving a nested directory for its children, so the paths are explicit.
#[path = "fixtures/environment.rs"]
mod environment;
#[path = "fixtures/ids.rs"]
mod ids;
#[path = "fixtures/rows.rs"]
mod rows;

pub(super) use environment::{
    CanaryEnvironment, mark_worker_stale, recover_orphaned_row, register_queue_worker,
    register_worker,
};
pub(super) use ids::{ClaimToken, JobId, QueueName, WorkerName};
pub(super) use rows::{
    Delivery, GuardedRowState, JobRow, RowState, claim_due_rows, delivery_task, fetch_one_task,
    fetch_task, insert_job, read_fetched_metadata, read_guarded_row, read_state,
};
