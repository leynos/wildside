# ADR-003: Durable retry scheduling for Apalis jobs

- Status: Accepted
- Date: 2026-10-17
- Deciders: Wildside maintainers

## Context and decision

In the context of background jobs with bounded retry budgets, facing a race
between in-process retry middleware and the PostgreSQL fetcher at the pinned
Apalis release candidates, we decided for a handler wrapper that schedules
retry delays durably in `apalis.jobs.run_at`, with exact `apalis-core` rc.9 and
`apalis-postgres` rc.8 pins and a one-hour ceiling on each delay, and against
in-process retry middleware and an aggregate backoff budget, to achieve retry
scheduling that releases worker capacity and preserves the attempt budget
across deliveries, accepting an extra guarded database write on failure and
timing precision limited by the worker poll interval.

The 2026-10-14 investigation found that in-process retries at the rc pins race
with the storage fetcher: a failed row can be fetched while the retry layer is
still processing it. The wrapper updates a row only while it is `Running` and
locked by the current worker, before Apalis acknowledges the failure. A
terminal outcome can also reduce the stored attempt limit so a later `Failed`
row is unfetchable. A crash between the in-memory attempt increment and its
acknowledgement can lose that one in-flight attempt.

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
- The pinned acknowledgement and fetch contracts are upgrade prerequisites,
  rather than assumptions that can be carried over to 1.0 without evidence.
