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
overwriting the decision of the delivery that replaced it. A terminal outcome
reduces the stored attempt limit to the terminating attempt, either within its
reason write or, if that write fails, as one following bounded write, so a later
`Failed` row is unfetchable. A crash between the in-memory attempt increment
and its acknowledgement can lose that one in-flight attempt.

On the 1.0 upgrade
([issue #550](https://github.com/leynos/wildside/issues/550)), re-verify the
V8 canaries: fetch eligibility before and after `run_at`, the stored attempt
restored to a fetched task, and the acknowledgement leaving `run_at`,
`max_attempts`, and `metadata` untouched. Also re-evaluate the middleware
ordering before changing the no-retry-layer rule. The canaries and their
positive and negative cases are specified in the
[ExecPlan](execplans/5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.md).

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
