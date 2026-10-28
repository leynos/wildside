# ADR-004: Dead letters as Apalis rows behind a domain port

- Status: Accepted
- Date: 2026-10-25
- Deciders: Wildside maintainers

## Context and decision

In the context of inspecting and managing terminal background jobs, facing
Apalis-managed storage, ambiguous legacy failure states, and the need to retain
requeue provenance across acknowledgements, we decided for a domain
`DeadLetterQueue` backed by guarded operations on dead rows in `apalis.jobs`
and two namespaced metadata keys, and against a separate dead-letter table, a
discard marker, and automatic requeue without an operator decision, to achieve
inspectable terminal outcomes and requeues with the queue's current retry
budget, accepting dependence on the pinned Apalis row and acknowledgement
contracts and the need to manage retained rows.

Dead rows are `Killed`, or `Failed` with a null limit or attempts at or above
the limit. The terminal update records `fatal` or `exhausted` under
`wildside.retry.dead_reason`; legacy and malformed reasons remain
`Unclassified`. A guarded requeue restores the queue policy's `max_attempts`,
resets attempts, increments the bounded `wildside.retry.requeue_count`, removes
the prior reason, and preserves unrelated metadata. Discard deletes one guarded
dead row. Listing exposes a bounded last error only with a recognized terminal
reason and a JSON-string error. The seeded `EXPLAIN` canary used the existing
index path, so the conditional partial index was not added.

On the 1.0 upgrade
([issue #550](https://github.com/leynos/wildside/issues/550)), re-verify the
V8 canaries for dead-row fetch exclusion, persisted attempts, acknowledgement
preservation of `run_at`, `max_attempts`, and `metadata`, and the
terminal-reason and requeue metadata transitions. The
[ExecPlan](execplans/5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.md)
records the positive and negative cases.

## Consequences

- Operators use the port or the guarded procedures in the
  [queue runbook](runbooks/queue-retry-and-dead-letter-operations.md); no
  automatic requeue policy is introduced.
- Requeue requires a configured policy for the row's queue and refuses invalid
  attempt or counter data. Legacy rows without a recognized reason are not
  assigned a guessed cause.
- Retention and pruning need an explicit owner and policy as queue history
  grows.
