# Debugging plan: durable probe provisioning

Generated: 2026-10-20. Scope: roadmap 5.2.3 EP-M0 setup failure. Falsification
agent: `alchemist`; the planning agent does not run the experiment.

## Problem and evidence

Two direct Cargo probe runs failed before the handler ran. Template
provisioning exhausted five attempts with
`failed to connect to admin database: db error`. The complete Makefile test
gate passed. The original SQLSTATE is unknown.

The Makefile exports a stable `PG_PASSWORD` and selects `target/pg_worker`. The
support helper resolves a default password for repair but deliberately does not
export it. The dependency formats the PostgreSQL error using Display, losing
the database error detail.

## H1: missing runner environment causes authentication failure

Claim: direct Cargo invocation omitted the stable password supplied by Make, so
embedded PostgreSQL bootstrap and the admin client disagree on credentials.
Plausibility: high, given the Makefile's documented random-password failure
mode.

Prediction: the existing probe executable fails without `PG_PASSWORD`, while
supplying the Makefile password allows provisioning to complete. A direct admin
connection in a minimal scratch diagnostic reports SQLSTATE `28P01` in the
failing environment. Hold the worker path constant to isolate the password.

### Falsification procedure

1. Run only the existing probe executable's main test twice, sequentially, with
   the Makefile worker path fixed: first without `PG_PASSWORD`, then with its
   documented default. Capture each output.
2. If needed, create a minimal scratch integration test containing only shared
   cluster setup, a direct `postgres::Client::connect` with database-error code
   and message reporting, and template provisioning. Never print URLs or
   passwords. Run the same two environments, then remove the scratch source.
3. Report whether provisioning is reached and succeeds independently of later
   Apalis assertions. Failure in both environments with another SQLSTATE
   falsifies H1; revise the plan before testing another hypothesis.

## Boundaries and termination

Use only the supplied experiment; no full gates, dependency edits, process
signals, service management, or manual shared-directory cleanup. Preserve the
normal shared lock and compiler admission. Use `/tmp` for logs only. If the
existing executable is unavailable, use the minimal scratch test. Stop after
the paired comparison and report a falsified, not-falsified, or inconclusive
verdict. The five durable-scheduling predictions remain separate from this
provisioning diagnosis.

## Result

H1 was not falsified. The original probe executable and worktree worker were
absent, so the agent built the minimal scratch diagnostic. Both runs used the
same executable and `/home/leynos/.cargo/bin/pg_worker` (installed version
0.5.2), varying only `PG_PASSWORD`.

- Without the password: direct admin connection reported SQLSTATE `28P01`,
  `password authentication failed for user "postgres"`; template provisioning
  reproduced the original five-attempt failure (3.70 seconds).
- With the Makefile default: admin connection and template provisioning both
  succeeded (1.23 seconds).

Logs:

- `/tmp/auth-probe-wildside-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling-absent.out`
- `/tmp/auth-probe-wildside-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling-stable.out`

The scratch source was removed. This identifies the missing runner password as
sufficient to explain the setup failure. It does not exercise the five Apalis
predictions. The developer guide now records the environment for direct Cargo
runs. No dependency changes or shared-service intervention were needed.
