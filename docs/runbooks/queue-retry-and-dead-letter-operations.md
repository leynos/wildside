# Queue retry and dead-letter operations

This runbook describes the PostgreSQL operations behind the `DeadLetterQueue`
port at the pinned Apalis release candidates. Roadmap section 6 anticipates an
operator or admin surface; item 5.3.1 owns worker deployment and may add
worker-side tooling. Until an operator surface is available, use the port where
it is composed, or the guarded SQL below in a `psql` session connected to the
correct database. These examples never select job payloads.

## List terminal jobs

The dead predicate includes `Killed` rows and `Failed` rows whose limit is null
or exhausted. A missing or unrecognized terminal reason is `Unclassified`. The
adapter also omits rows with negative attempts or malformed requeue counts;
this diagnostic query shows those rows so an operator can investigate them.

```sql
SELECT id, job_type, attempts, max_attempts, done_at,
       CASE WHEN metadata->>'wildside.retry.dead_reason' IN ('fatal', 'exhausted')
            THEN metadata->>'wildside.retry.dead_reason'
            ELSE 'Unclassified' END AS reason,
       metadata->>'wildside.retry.requeue_count' AS requeue_count,
       CASE WHEN metadata->>'wildside.retry.dead_reason' IN ('fatal', 'exhausted')
                 AND jsonb_typeof(last_result->'Err') = 'string'
            THEN left(last_result->>'Err', 256) END AS last_error
FROM apalis.jobs
WHERE status = 'Killed'
   OR (status = 'Failed' AND (max_attempts IS NULL OR attempts >= max_attempts))
ORDER BY done_at DESC NULLS LAST, id DESC
LIMIT 100;
```

The SQL example limits the diagnostic to 256 characters; the adapter limits it
to 1,024 UTF-8 bytes at a character boundary. A recognized reason and a
JSON-string `Err` are both required before either surface exposes an error.

## Requeue one dead job

Correct the underlying fault first. Find the row's queue name and obtain that
queue's **current configured** `JobRetryPolicy.max_attempts`; do not derive the
budget from the dead row, whose limit may have been reduced on a fatal failure.
The adapter refuses an unknown queue, negative attempts, or an invalid counter.
It increments a valid counter with saturation at `u32::MAX` and preserves other
metadata. The following transaction applies the same guards. Enter the exact
queue name and an integer policy budget in the PostgreSQL `integer` range.

```sql
\prompt 'Dead job id: ' job_id
\prompt 'Configured queue name: ' job_type
\prompt 'Current queue max attempts: ' retry_budget
BEGIN;
WITH candidate AS (
    SELECT id, job_type, attempts,
           metadata->>'wildside.retry.requeue_count' AS raw_count
    FROM apalis.jobs
    WHERE id = :'job_id' AND job_type = :'job_type'
      AND (status = 'Killed' OR
           (status = 'Failed' AND (max_attempts IS NULL OR attempts >= max_attempts)))
    FOR UPDATE
), normalized AS (
    SELECT *, CASE WHEN raw_count IS NULL THEN '0'
                   WHEN raw_count ~ '^[+]?[0-9]+$'
                   THEN coalesce(nullif(ltrim(raw_count, '+0'), ''), '0')
              END AS digits
    FROM candidate
), parsed AS (
    SELECT *, CASE WHEN length(digits) <= 20 THEN digits::numeric END AS count_value
    FROM normalized
)
UPDATE apalis.jobs AS jobs
SET status = 'Pending', attempts = 0, max_attempts = :'retry_budget'::integer,
    run_at = now(), lock_by = NULL, lock_at = NULL,
    metadata = jsonb_set(
        coalesce(jobs.metadata, '{}'::jsonb) - 'wildside.retry.dead_reason',
        '{wildside.retry.requeue_count}',
        to_jsonb(least(parsed.count_value + 1, 4294967295)::bigint), true)
FROM parsed
WHERE jobs.id = parsed.id AND jobs.job_type = parsed.job_type
  AND parsed.attempts >= 0
  AND parsed.count_value BETWEEN 0 AND 18446744073709551615
  AND :'retry_budget'::bigint BETWEEN 1 AND 2147483647
  AND (jobs.status = 'Killed' OR
       (jobs.status = 'Failed' AND (jobs.max_attempts IS NULL OR jobs.attempts >= jobs.max_attempts)))
RETURNING jobs.id, jobs.job_type, jobs.attempts, jobs.max_attempts, jobs.run_at,
          jobs.metadata->>'wildside.retry.requeue_count' AS requeue_count;
```

Check that exactly one row is returned, then run `COMMIT;`. If no row is
returned, run `ROLLBACK;` and investigate the identity, queue policy, status,
attempt count, and counter. A malformed or out-of-range counter and negative
attempts do not mutate the row. An invalid budget or incompatible metadata can
raise a SQL error; roll back the transaction. Requeue removes the old terminal
reason but leaves unrelated metadata and `last_result` intact, matching the
adapter.

## Discard one dead job

Discard deletes the row. Verify the identifier and retain any required audit
record before executing it. A live or missing row returns no result.

```sql
\prompt 'Dead job id: ' job_id
DELETE FROM apalis.jobs
WHERE id = :'job_id'
  AND (status = 'Killed' OR
       (status = 'Failed' AND (max_attempts IS NULL OR attempts >= max_attempts)))
RETURNING id, job_type;
```

## Detect repeated worker crashes

At `apalis-postgres` rc.8, orphan recovery changes a stale `Running` or
`Queued` row to `Pending`, clears its lock and completion time, increments
`attempts`, and writes the exact error below. Capture successive snapshots and
correlate them with worker heartbeat and restart logs:

```sql
SELECT id, job_type, status, attempts, run_at, lock_by, lock_at
FROM apalis.jobs
WHERE status IN ('Pending', 'Queued', 'Running')
  AND last_result->>'Err' = 'Re-enqueued due to worker heartbeat timeout.'
ORDER BY job_type, id;
```

Repeated sightings of the same job with rising attempts, together with worker
restarts or missed heartbeats, indicate a possible crash loop. One sighting can
be ordinary orphan recovery. A later acknowledgement replaces `last_result`, so
a missing marker in a later snapshot does not establish that a worker was
healthy. Investigate the worker and the underlying fault before requeueing a
terminal job. Retention and pruning of accumulated history need a separate
operational policy; see roadmap section 6.
