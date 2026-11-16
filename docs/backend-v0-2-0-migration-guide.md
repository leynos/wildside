# Backend v0.2.0 migration guide (from v0.1.0)

This guide covers the changes that ship as `backend` v0.2.0: durable retry
scheduling for Apalis jobs, dead letters behind a domain port, and a provider
constructor that supplies each stored job's attempt budget. As with the
repository's other migration guides, breaking changes come first, followed by
new features that need a usage change to realize their benefit.

## Scope and audience

This guide is for code that embeds the `backend` crate: composition roots that
build the Apalis provider, register queue workers, or operate the job table.
The crate is pre-1.0, so the breaking changes ship without compatibility shims
or deprecated overloads; each one is a compile error that names the changed
signature.

The binaries in this repository already carry the changes. These steps matter
when a downstream consumer, a maintained fork, or a long-lived branch picks up
the new revision.

## Quick upgrade checklist

- Pass a queue name and its `JobRetryPolicy` to
  `ApalisPostgresProvider::new`.
- Return `Result<(), JobFailure>` from queue handlers, classifying each
  failure with `JobErrorKind`.
- Build workers with `RetryingJobService` and `RetryingJobServiceConfig`.
- Register `PostgresDeadLetterQueue` with per-queue `RequeueBudgets` wherever
  dead-letter operations are exposed.
- Wire the retry counters into alerting before enabling a worker.

## Breaking changes

### `ApalisPostgresProvider::new` takes a queue name and retry policy

The constructor gained the queue name and the queue's policy so the attempt
budget stored on each job comes from the policy rather than from a second value
that could drift. It remains `async` and returns
`Result<Self, JobDispatchError>`.

```rust
use backend::domain::jobs::retry::{JobRetryPolicy, JobRetryPolicyConfig};
use backend::outbound::queue::ApalisPostgresProvider;
use sqlx::PgPool;
use std::{num::NonZeroU32, time::Duration};

async fn build_provider(
    pool: PgPool,
) -> Result<ApalisPostgresProvider, Box<dyn std::error::Error>> {
    let policy = JobRetryPolicy::new(JobRetryPolicyConfig {
        max_attempts: NonZeroU32::new(5).expect("the limit is positive"),
        base_delay: Duration::from_secs(2),
        multiplier: 2.0,
        max_delay: Duration::from_secs(120),
        jitter_fraction: 0.1,
    })?;

    let provider =
        ApalisPostgresProvider::new(pool, "route.generate", policy).await?;
    Ok(provider)
}
```

A policy limit that does not fit Apalis's signed `integer` attempt column is
rejected with `JobDispatchError::Rejected`.

### Handler failures are classified values

`RetryingJobService` serves a queue whose handler reports its own retryability.
The service is generic over a job type `J` and a handler `F` whose future
resolves to `Result<(), JobFailure>`:

```text
F: Fn(J, NonZeroU32) -> Fut
Fut: Future<Output = Result<(), JobFailure>>
```

The second handler argument is the current delivery attempt. Build each failure
with `JobFailure::new(kind, message)`, where `kind` is a `JobErrorKind`:

| `JobErrorKind` | Meaning                                                   |
| -------------- | --------------------------------------------------------- |
| `Retryable`    | The delivery may succeed if tried again later.            |
| `Fatal`        | The delivery will not succeed if retried; dead-letter it. |

The message must be a payload-free diagnostic, because it is written to the
queue's plain last-result field. `JobFailure::new` truncates it at a UTF-8
boundary to `MAX_JOB_FAILURE_MESSAGE_BYTES`, so a long upstream message cannot
overflow that field. Decide retryability where the cause is known: a handler
that collapses every error into one value cannot distinguish a transient outage
from a malformed payload.

### Jobs carry a stored attempt budget

Changing the policy does not rewrite rows already in the table. Each stored job
keeps the `max_attempts` recorded when it was enqueued, and a new budget
applies only to jobs enqueued afterwards.

The wrapper still applies the current policy on each failed delivery, so an
older row with a larger stored cap is dead-lettered once the policy's limit is
reached; when that terminating attempt is below the stored cap, the cap is
lowered to it. A row whose stored cap is below the current policy limit
dead-letters at its own cap instead, so raising the policy does not revive a
row that was enqueued with a smaller budget.

## New ports and composition

### Retry wrapper

`RetryingJobServiceConfig::new` takes the queue name, the same policy the
provider received, a `RetryPersistence` implementation, and the queue's
`RouteQueueMetrics`. `RetryingJobService::new` then pairs that configuration
with the handler:

```rust
use std::sync::Arc;

use backend::outbound::queue::{
    JitterSource, PostgresRetryScheduler, RetryingJobService,
    RetryingJobServiceConfig,
};

let config = RetryingJobServiceConfig::new(
    queue_name,
    queue_policy.clone(),
    Arc::new(PostgresRetryScheduler::new(pool.clone())),
    metrics,
);
let service = RetryingJobService::<GenerateRouteJob, _>::new(config, handle_job);
let worker = WorkerBuilder::new(worker_name)
    .backend(postgres_storage)
    .build(service);
```

Pass the same policy to the provider and to the wrapper so the stored budget
and the wrapper's decisions agree. Jitter comes from a `JitterSource`, which
defaults to `ThreadRngJitter`; replace it with
`RetryingJobServiceConfig::with_jitter` in tests so scheduled delays are
reproducible without depending on a zero jitter fraction.

### Dead letters

`PostgresDeadLetterQueue::new` takes the pool and the attempt limits to restore
on requeue. Build those limits from the same policies the enqueue adapters use:

```rust
use backend::outbound::queue::{PostgresDeadLetterQueue, RequeueBudgets};

let dead_letters = PostgresDeadLetterQueue::new(
    pool.clone(),
    RequeueBudgets::from_policies([
        ("route.generate", route_policy.clone()),
        ("enrichment.source", enrichment_policy.clone()),
    ]),
);
```

The `DeadLetterQueue` port exposes three operations:

- `list_dead` returns the newest dead jobs first. Rows it cannot read are
  reported with their identifier and the field that made them unreadable,
  rather than dropped silently.
- `requeue` restores the queue's configured attempt budget, clears the
  dead-letter reason, and writes `run_at` from the database clock so the row is
  immediately eligible. It rejects a job whose queue has no entry in
  `RequeueBudgets`, so a requeued row always carries a budget the process knows
  about.
- `discard` permanently removes a dead job.

Dead jobs report their own terminal reason: `DeadReason::Fatal` for a failure
the handler classified as permanent, `DeadReason::Exhausted` for a spent
attempt budget, and `DeadReason::Unclassified` for a legacy or unrecognized
row. Unclassified rows stay listed so an upgrade does not strand them.

## Observability before enabling workers

The retry path emits three counters. Alerting must cover them before a worker
runs, because the dead-letter counter alone will not show a terminal write that
never applied:

| Counter                                               | Raised when                             |
| ----------------------------------------------------- | --------------------------------------- |
| `route_queue_retry_total{outcome="schedule_failure"}` | A retry write was refused or failed.    |
| `route_queue_dead_letter_total`                       | A terminal failure was recorded.        |
| `route_queue_terminal_write_failure_total`            | A terminal write was refused or failed. |

Both failure paths also log a warning whose `decision` field is
`schedule_failure` or `terminal_write_failure`. The dead-letter backlog itself
is a database fact rather than an exported gauge; query it with the read-only
statement in the
[dead-letter runbook](runbooks/queue-retry-and-dead-letter-operations.md).

## Verification

After migrating, run the queue and dead-letter behavioural suites against a
test database, then confirm on staging that a retryable failure schedules a
later `run_at`, a fatal failure dead-letters on its first delivery, and a
requeued row is picked up by the worker that owns the queue.

## See also

- [Wildside server users guide](users-guide.md#job-retries-and-dead-letters)
  for the retry, dead-letter, and counter semantics.
- [Queue retry and dead-letter operations](runbooks/queue-retry-and-dead-letter-operations.md)
  for the operator procedure.
- [ADR 004](adrs/adr-0004-durable-retry-scheduling-for-apalis-jobs.md) for
  durable backoff and the pinned worker contract.
- [ADR 005](adrs/adr-0005-dead-letters-as-apalis-rows-behind-a-domain-port.md)
  for terminal jobs, guarded transitions, and requeue provenance.
