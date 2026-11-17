# Architecture Decision Record (ADR) 004: Durable retry scheduling for Apalis jobs

## Status

Accepted — 2026-10-17: schedule retry delays durably in `apalis.jobs.run_at`
from a handler wrapper, with exact `apalis-core` rc.9 and `apalis-postgres`
rc.8 pins, instead of installing an in-process retry layer.

## Date

2026-10-17

## Context

Background jobs carry bounded retry budgets, and a retry has to preserve the
attempt count the job was given. The 2026-10-14 investigation found that
in-process retry middleware at the pinned release candidates races the
PostgreSQL fetcher: a failed row can be fetched again while the retry layer is
still processing it. An aggregate backoff budget would hide the remaining
attempts from the row, so neither alternative can be made safe at these pins.

## Decision

Schedule each retry delay durably in `apalis.jobs.run_at` from a handler
wrapper, with a one-hour ceiling on each delay.

The wrapper updates a row only while it is `Running`, locked by the current
worker, and still carrying the attempt count this delivery inherited; the last
predicate is what keeps a handler that stalled through orphan recovery from
overwriting the decision of the delivery that replaced it. That guard covers
the wrapper's own writes only. The acknowledgement Apalis performs afterwards
is not guarded by row status or attempt count: `ack.sql` at the pinned release
matches on `id` and `lock_by` alone and then overwrites `status`, `attempts`,
and `last_result`. A stale delivery whose replacement is locked under the same
worker name can therefore overwrite the replacement's `Running` row with its
own attempt count and a terminal status while the replacement still runs. This
repository does not control `ack.sql`. The mitigation is a distinct worker name
per worker instance, which makes a stale acknowledgement match no row at all;
the risk returns to review at the 1.0 upgrade described below.

A terminal outcome reduces the stored attempt limit to the terminating attempt,
either within its reason write or, if that write fails, as a short bounded
sequence of fallback writes, so a later `Failed` row is unfetchable. The
fallback is retried because it runs only after another write has already
failed, which usually means a transiently failing database; a fallback that
never lands is logged as `terminal_cap_clamp_exhausted`, because a database
that refuses every write leaves no way for a handler to protect the row. A
crash between the in-memory attempt increment and its acknowledgement can lose
that one in-flight attempt.

The reduced cap also has to hold against the fetch predicate, which accepts a
`Pending` row without consulting `max_attempts`. Orphan recovery returns a
retired row to `Pending` and increments `attempts`, so a row retired between
its terminal write and its acknowledgement can become deliverable once more.
The wrapper therefore refuses a delivery whose attempt count already exceeds
the row's stored `max_attempts`, without invoking the handler; the
acknowledgement that follows records a delivery that never ran.

On the 1.0 upgrade
([issue #550](https://github.com/leynos/wildside/issues/550)), re-verify the
V8 canaries: fetch eligibility before and after `run_at`, the stored attempt
restored to a fetched task, and the acknowledgement leaving `run_at`,
`max_attempts`, and `metadata` untouched. Also re-evaluate the middleware
ordering before changing the no-retry-layer rule. The canaries and their
positive and negative cases are specified in the
[ExecPlan](../execplans/5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.md).

## Consequences

- Each queue supplies its own validated `JobRetryPolicy`; the worker installs
  `RetryingJobService` without an Apalis retry layer.
- Backoff does not occupy a worker slot. Delivery can occur after the due time
  by up to the poll interval or longer under load.
- The retry deadline is computed from the worker's `ClockSource` and bound as
  an absolute `run_at`, which keeps the stored delay reproducible in tests. The
  accepted risk is clock skew: Apalis compares that application-clock instant
  with the database's `now()`, so a worker clock behind the database makes a
  retry eligible early, and a clock ahead delays it. Pool-connection wait after
  the deadline is computed also shortens the effective delay. Mitigate by
  synchronizing hosts with a time service and monitoring clock offset; the
  fixed-clock tests verify the stored delay, not eligibility.
- The pinned acknowledgement and fetch contracts are upgrade prerequisites,
  rather than assumptions that can be carried over to 1.0 without evidence.
