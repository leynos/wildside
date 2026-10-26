# Implement retry policies with exponential backoff and dead-letter handling (backend 5.2.3)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
`Outcomes & Retrospective`, `Conformance Basis`, and `Verification Plan` must
be kept up to date as work proceeds.

Status: IN PROGRESS (EP-M0, EP-M1, and EP-M2 complete; EP-M3 active)

Unqualified calendar dates in this plan use the `Europe/Berlin` timezone.
Review metadata may use UTC and therefore show the previous calendar date.

This plan covers backend roadmap item 5.2.3 only:

> Implement retry policies with exponential backoff and dead-letter handling.

The user approved this plan and authorized execution on 2026-10-13. On
2026-10-17 the user accepted the revised mechanism (D10: exact Apalis pins;
D11: durable, storage-scheduled backoff). This approval does not cover worker
consumption or deployment (roadmap 5.3.1), trace-identifier propagation
(5.2.4), the Apalis 1.0.0 upgrade (issue
[#550](https://github.com/leynos/wildside/issues/550)), or request-path
dispatch changes. The `TODO(#276)` markers in
`backend/src/domain/route_submission/mod.rs` stay in place.

## Read this first (revision of 2026-10-25)

For readers of an earlier version, these are the changes that matter:

- There is **no tower retry layer**. Do not add the `apalis` facade crate, and
  do not call `.retry(...)` on any `WorkerBuilder`. At the pinned versions an
  in-process retry layer races with the storage fetcher (see
  `Surprises & discoveries`, 2026-10-14).
- Backoff is **durable**. When a job fails, a handler wrapper writes the next
  `run_at` for a retry, or shrinks `max_attempts` for a terminal failure on the
  still-locked row *before* the ack runs. The library's ack then records
  `Failed` or `Killed`, and the existing fetch predicate does the rest
  (Decision D11). Accepted D15 also records a namespaced terminal reason in
  JSONB `metadata` because those statuses alone cannot distinguish fatality
  from exhaustion when the current policy is lower than a stored limit.
- Apalis is **pinned exactly**, to `apalis-core = "=1.0.0-rc.9"` and
  `apalis-postgres = "=1.0.0-rc.8"` (Decision D10). The 1.0.0 upgrade is issue
  #550, which is out of scope here.
- AXIOM-FACADE and AXIOM-ATTEMPT are retired. AXIOM-PIN, AXIOM-DURABLE, and
  AXIOM-ACK replace them, and EP-M0 discharges them.
- `TOTAL_BACKOFF_BUDGET` is retired. Workers no longer hold a slot during
  backoff, so the per-delay cap `MAX_BACKOFF_DELAY` replaces it (Decision D13).
- `requeue` must restore the queue's configured `max_attempts` because a
  terminal path may shrink it (Decision D12).
- EP-M1 (the pure domain policy) is unaffected in shape. EP-M2 has been
  rewritten. EP-M3 gains the budget restoration. EP-M4 names the renamed ADRs.

## Purpose / big picture

Wildside enqueues background jobs (`GenerateRouteJob`, `EnrichmentJob`) through
the `RouteQueue` port into an Apalis-backed PostgreSQL queue (`apalis.jobs`
table). Today a job that fails during processing would be redelivered using
Apalis defaults: up to 25 attempts, with no deliberate backoff. A permanently
failing job would linger with no dead-letter story. Nothing bounds retries
deliberately, nothing classifies errors as retryable or fatal, and no interface
exists to inspect or requeue dead jobs.

This plan adds the retry and dead-letter policy layer:

- A pure domain retry policy. It computes bounded exponential backoff delays
  with jitter and decides, per attempt and error class, whether to retry a job
  or dead-letter it. The domain policy is the policy production executes: the
  adapter's handler wrapper calls it directly, so the verified function and the
  running function are the same function.
- A domain error classification for job execution failures (retryable or
  fatal), following the precedent of `EnrichmentSourceError::is_retryable()`.
- Durable retry scheduling in the outbound queue adapter. A handler wrapper
  turns each failure into a guarded update on the still-locked `apalis.jobs`
  row: a future `run_at` for a retry, or a shrunken `max_attempts` for a
  terminal failure. The library's own ack and fetch predicate then enforce the
  backoff and the dead state.
- A `DeadLetterQueue` driven port and a PostgreSQL adapter that lists,
  requeues, and discards dead jobs, with guarded state transitions, distinct
  fatal and exhausted reasons for rows with D15 metadata, an unclassified
  fallback for legacy rows, restoration of the retry budget on requeue, and
  durable requeue provenance.
- Enqueue-side persistence of the policy's `max_attempts` on every job, and
  retry and dead-letter event counters through dedicated `RouteQueueMetrics`
  methods.

Observable outcomes:

- `cargo test -p backend` demonstrates the following against embedded
  PostgreSQL via `rstest-bdd` scenarios:
  - A job whose handler always fails retryably runs exactly `max_attempts`
    times in total across storage redeliveries. Each redelivery happens no
    earlier than the `run_at` the previous failure scheduled, and the job then
    ends `Killed`.
  - A fatal failure runs the handler exactly once and leaves a row that the
    storage fetcher can never pick up again.
  - A dead job can be listed and requeued through the `DeadLetterQueue` port,
    regains its full retry budget, and then observably runs to success.
  - Requeuing a live or absent job fails cleanly.
- Property tests pin the backoff invariants: delays bounded above and below,
  jitter within the configured fraction, monotone non-decreasing growth until
  the cap binds, and retry decisions that never exceed `max_attempts`.
- `make check-fmt`, `make typecheck`, `make lint`, and `make test` pass after
  each major milestone.
- Roadmap item 5.2.3 is marked done only after every gate passes and the
  documentation updates land.

## Context and orientation

Everything below is verifiable in the working tree. No prior plan is required
reading, though
`docs/execplans/backend-5-2-2-job-structs-for-generate-route-and-enrichment.md`
records the scope boundaries this plan inherits.

Repository state relevant to this task:

- Port: `backend/src/domain/ports/route_queue.rs` defines `RouteQueue` (one
  method, `enqueue`) and `JobDispatchError` (`Unavailable`, `Rejected`) via the
  `define_port_error!` macro in `backend/src/domain/ports/macros.rs`.
  `JobDispatchError` has no retryable/fatal distinction; it describes
  enqueue-side failures only.
- Adapter: `backend/src/outbound/queue/apalis_route_queue.rs` implements the
  enqueue side. `ApalisPostgresProvider` wraps
  `apalis_postgres::PostgresStorage<serde_json::Value>` over a dedicated
  `sqlx::PgPool`, and `setup` creates the `apalis.jobs` schema at runtime (no
  Diesel migration). `push_job` currently calls `storage.push(payload)`, which
  uses the default context and therefore `max_attempts = 25`. `QueueProvider`
  is the crate-internal test seam, with `FakeQueueProvider` and
  `FailingQueueProvider` doubles in
  `backend/src/outbound/queue/test_helpers.rs`.
- Consumer-side scaffolding: `backend/src/outbound/queue/job_decode.rs`
  provides the pure `decode_job<J>` helper. Its module documentation and
  `docs/wildside-backend-architecture.md` (section "Background Job Execution
  (Apalis Workers)") both state that the queue-processing error path applies
  "the retry/dead-letter policy owned by 5.2.3". That seam is where this plan's
  handler wrapper attaches. `decode_job` deliberately never logs payload
  contents, and the same privacy stance applies to every retry and dead-letter
  surface in this plan. A decode failure is the canonical fatal error.
- Jobs: `backend/src/domain/jobs/generate_route.rs` and
  `backend/src/domain/jobs/enrichment.rs` define versioned V1 envelopes. The
  module documentation of `backend/src/domain/jobs/mod.rs` currently scopes the
  module to job payloads. It must be updated when the retry policy joins it.
- Error-classification precedent:
  `backend/src/domain/ports/enrichment_source.rs` exposes
  `EnrichmentSourceError::is_retryable()`, and
  `backend/src/domain/overpass_enrichment_worker/attempt_error.rs` shows the
  house pattern for attempt-local error taxonomies.
- Metrics: the `RouteQueueMetrics` port with `RouteQueueOutcome`
  (`success`/`failure`) and the Prometheus adapter exist. Enqueue outcomes and
  latency remain enqueue-only. The port gains separate retry and dead-letter
  event counters (Decision D8). Queue-depth and age gauges remain deferred to
  roadmap 6.x.
- Randomness: `rand = { version = "0.8", features = ["small_rng"] }` is
  already a production dependency in `backend/Cargo.toml`. The adapter draws
  jitter samples from it, so no new dependency is needed.
- No worker, `Monitor`, or `WorkerBuilder` exists anywhere in the crate, and
  nothing configures Apalis orphan recovery (`reenqueue_orphaned_after`) yet.
  Production worker consumption and orphan-recovery configuration belong to
  roadmap 5.3.1. Until 5.3.1 lands, a worker process that dies mid-job leaves
  the row `Running` indefinitely. This plan documents that fact rather than
  fixing it. Test-only consumption harnesses are in scope, because behavioural
  evidence of retry and dead-letter behaviour requires driving job execution in
  tests.

Apalis versions: the current `backend/Cargo.toml:34-35` pins
`apalis-core = "=1.0.0-rc.9"` and `apalis-postgres = "=1.0.0-rc.8"`, matching
`Cargo.lock`. The earlier caret requirements (`"1.0.0-rc.7"` and
`"1.0.0-rc.6"`) also admitted the breaking `1.0.0` final release; EP-M0
replaced them with exact pins on the resolved versions (Decision D10). All
facts below are verified against rc.9/rc.8, the code that actually compiles.
The cached sources are under
`~/.cargo/registry/src/*/apalis-{core-1.0.0-rc.9,postgres-1.0.0-rc.8,sql-1.0.0-rc.9}`.
They are external axioms for this plan:

- Worker middleware order (`apalis-core` `src/worker/mod.rs`), outermost
  first: `Data`, user middleware, backend middleware (`Ack` around `LockTask`),
  `Readiness`, `Tracker`, then the handler. The tracker's `AttemptOnPollFuture`
  increments the task's `Attempt` on first poll, so inside the handler
  `Attempt::current()` is the 1-based number of the current attempt.
- Fetch (`apalis.get_jobs`, migration `20251018165056_queue_jobs.sql`) selects
  rows with
  `status = 'Pending' OR (status = 'Failed' AND attempts < max_attempts)` and
  `run_at < now()`, and marks them `Queued`. `LockTask`
  (`queries/task/lock_by_id.sql`) moves a row to `Running` and sets `lock_by`
  to the worker name, again only when `run_at < now()`. The `queue_by_id.sql`
  path also requires `run_at < now()`. The notify trigger fires only on
  `INSERT`. **Every fetch path therefore honours `run_at`**, which is the
  property D11 relies on.
- Hydration (`apalis-sql` `src/from_row.rs`): a fetched row becomes a task
  whose `Attempt` starts at the persisted `attempts` and whose context
  `max_attempts` is the persisted `max_attempts` (default 25 if null). This
  fallback happens only after fetch; `get_jobs` compares
  `attempts < max_attempts`, so a nullable legacy `Failed` row is not fetched
  at all.
- Ack (`apalis-postgres` `src/ack.rs`, `queries/task/ack.sql`): after the
  handler returns, the ack writes `status`, `attempts = Attempt::current()`,
  `last_result`, and `done_at`, guarded by `id` and `lock_by`. It never touches
  `run_at`, `max_attempts`, or the JSONB `metadata` column. `calculate_status`
  returns `Done` on `Ok`, and `Killed` on `Err` when the *in-memory* context
  `max_attempts` is at most `Attempt::current()`; otherwise it returns
  `Failed`. The `Error::Abort => Killed` arm is commented out, so abort errors
  are not persisted as `Killed`.
- Extractors (`apalis-core`): `Attempt`, `TaskId`, and `WorkerContext`
  implement `FromRequest`, so a handler can obtain the attempt number, the task
  id, and the worker name (`WorkerContext::name()`, which is the value
  `LockTask` writes to `lock_by`).
- Enqueue context: `PgContext` (`apalis_sql::context::SqlContext<PgPool>`)
  has `with_max_attempts(i32)`, and `TaskSink::push_task` accepts a built task.
  The adapter can therefore persist a per-job `max_attempts` without importing
  `apalis-sql` directly.
- There is no built-in dead-letter table or requeue API. Dead jobs are rows
  that remain in `apalis.jobs`. Listing and requeuing require direct SQL. No
  pruning or retention mechanism exists upstream at these pins.
- The `apalis` facade crate's `RetryLayer` exists, but this plan does not use
  it (Decision D14). Installed as user middleware at these pins, it re-enters
  the ack on every in-process attempt. The ack leaves a re-fetchable `Failed`
  row while the layer sleeps, so another worker can run the same job
  concurrently.

Named external axioms, each discharged in EP-M0 and then guarded permanently by
the V8 canaries. The upstream-surprises tolerance attaches to each:

- AXIOM-PIN: after EP-M0's pin change, `cargo tree -p backend` resolves
  exactly `apalis-core 1.0.0-rc.9` and `apalis-postgres 1.0.0-rc.8`, and no
  other Apalis release.
- AXIOM-DURABLE: a handler that updates its own locked row's `run_at` (or
  `max_attempts`) before returning `Err` controls redelivery. The row is not
  fetched before `run_at`; it is fetched after it; the next run observes
  `Attempt::current()` one higher; exhaustion persists `Killed`; and a row whose
  `max_attempts` was shrunk to the current attempt is never fetched again.
  EP-M0's runtime probe tests these five predictions.
- AXIOM-ACK: the ack writes only `status`, `attempts`, `last_result`, and
  `done_at`, guarded by `id` and `lock_by`, and computes status from the
  in-memory context as described above. It leaves `metadata` unchanged.

Terms used in this plan:

- "Dead-letter" a job: stop retrying it and park it in a state the storage
  fetcher can never redeliver, for operator inspection and optional requeue.
  Dead rows satisfy this predicate:

  ```sql
  status = 'Killed'
  OR (status = 'Failed' AND (max_attempts IS NULL OR attempts >= max_attempts))
  ```

  The pinned PostgreSQL migration declares `max_attempts` non-null, but Apalis
  hydration accepts null and defaults it to 25 only after a row is fetched. The
  fetch query's `attempts < max_attempts` comparison cannot select a row with a
  null limit, regardless of its attempt count. The listing therefore includes
  every nullable legacy `Failed` row so it remains visible for operator
  inspection and optional requeue; without a recognized reason key, it is
  `Unclassified`. D15's namespaced metadata key records whether a
  wrapper-originated terminal failure was fatal or exhausted. Rows without a
  recognized reason key are `Unclassified` because status and attempts cannot
  reliably distinguish their terminal cause.
- "Fatal error": a job-execution failure that retrying cannot fix (for
  example, a payload that fails decoding or validation). It is classified so
  the job dead-letters immediately rather than burning attempts.
- "Retryable error": a transient failure (timeout, transport, rate limit)
  worth retrying with backoff.
- "Handler wrapper": the adapter-owned service passed to
  `WorkerBuilder::build`. It runs the job-specific handler, classifies its
  failure, consults `JobRetryPolicy`, applies the guarded scheduling update,
  records metrics and logs, and then returns the error so the library's ack
  persists it.
- "Attempt": one handler invocation. Attempts are persisted by the ack after
  every invocation, so `max_attempts` is a lifetime bound. The one exception is
  an invocation that crashes the worker process before its ack (see `Risks`).

## Constraints

Hard invariants. Violation requires escalation, not workarounds.

- Do not begin a milestone before the previous plateau is committed and its
  gates pass. D10 and D11 are approved; any further mechanism change needs the
  user's approval.
- Keep scope to roadmap item 5.2.3. Do not implement 5.2.4 (trace
  propagation), 5.3.1 (worker pools, Kubernetes deployment, queue partitioning,
  orphan-recovery configuration), the Apalis 1.0.0 upgrade (#550), or
  request-path dispatch. Production binaries must not gain a worker loop from
  this plan; consumption harnesses are test-only.
- Preserve hexagonal architecture. The retry policy, error classification,
  and job failure type live in the domain layer and must not import Apalis,
  tower, SQLx, Diesel, or Actix types. The `DeadLetterQueue` port is defined by
  the domain, and the PostgreSQL adapter implements it in
  `backend/src/outbound/`. The handler wrapper and its scheduling SQL are
  adapter code in `backend/src/outbound/queue/`. Background workers and caches
  interact with the domain exclusively via ports (roadmap section 5 preamble).
- The domain policy is authoritative. Every retry or dead-letter outcome in
  production must come from `JobRetryPolicy::decide`. No other code may compute
  a delay or a retry decision.
- Do not install any in-process retry middleware on an Apalis worker: no
  `apalis` facade dependency, no `RetryLayer`, no `.retry(...)`, and no
  hand-rolled tower `Retry` (Decision D14). EP-M2 adds a conformance check for
  this.
- Every scheduling update written by the handler wrapper must be guarded by
  `id`, `lock_by = <this worker's name>`, and `status = 'Running'`, so it can
  only affect the row this worker currently holds. An update that affects zero
  rows is logged at `warn` level and never retried.
- The retry policy constructor must reject any configuration whose capped
  delay exceeds `MAX_BACKOFF_DELAY`, initially one hour (Decision D13).
- Do not change the `RouteQueue` trait signature. `JobDispatchError`
  variants may only be extended, not changed, and only if a milestone
  demonstrates the need. The default position is to leave enqueue-side errors
  untouched, because retry classification concerns job execution, not enqueue.
- Dependencies: pin `apalis-core = "=1.0.0-rc.9"` and
  `apalis-postgres = "=1.0.0-rc.8"` exactly (D10). Do not upgrade or replace
  them, `sqlx`, or `rand`. `googletest` is the single permitted dev-dependency
  addition. No production dependency may be added.
- Do not alter or migrate the Apalis-owned schema (`apalis.jobs` columns,
  statuses, functions), and do not add Diesel migrations for Apalis tables. The
  scheduling and dead-letter adapters may *write* existing columns (`run_at`,
  `max_attempts`, `status`, `attempts`, `last_result`) through guarded
  statements. Accepted D15 authorizes guarded writes to the existing JSONB
  `metadata` column for the namespaced requeue counter and terminal reason,
  preserving every other key. The user accepted these two additional values on
  2026-10-25; no other Apalis metadata writes are authorized. One additive
  exception is pre-authorized: the dead-letter adapter may create a partial
  index supporting the dead-jobs predicate, through the adapter's setup path
  alongside `PostgresStorage::setup`, if and only if milestone EP-M3's
  `EXPLAIN` check shows a sequential scan (Decision D6). A separate dead-letter
  table is out of scope (Decision D3).
- Dead-letter state transitions must be guarded compare-and-swap updates.
  `requeue` and `discard` take effect only when the row currently satisfies the
  dead predicate, and an ineffective update surfaces as a port error, never as
  silence.
- Automated requeue is forbidden. `requeue` is an operator action, and any
  future automation must carry a per-job requeue budget (recorded in ADR-003).
  Accepted D15 records requeue provenance so repeat offenders are visible.
- Retry and dead-letter surfaces (the `DeadJob` type, the error text the
  wrapper returns to the ack, logs, and metrics) must not expose raw job
  payloads. Error strings must be size-bounded. This matches the existing
  `decode_job` privacy stance.
- Behavioural tests may assert counts, orderings (including comparisons
  between persisted timestamps), and terminal states. They must not assert
  wall-clock magnitudes. The test consumption harness must have a hard
  iteration and deadline cap, so a wiring bug fails fast instead of hanging the
  suite. Test policies use millisecond-scale delays and a short poll interval.
- Versioned job envelopes and their JSON shape (pinned by `insta`
  snapshots from 5.2.2) must not change.
- Keep documentation in en-GB-oxendict style per
  `docs/documentation-style-guide.md`: wrap prose at 80 columns and code at 120
  columns, and use sentence-case headings.
- Keep source files below 400 lines. Split modules before finishing a
  milestone if needed.
- Run gates sequentially (build caching), never in parallel, and capture
  long output with `tee` to
  `/tmp/$ACTION-wildside-$(git branch --show-current).out`. Prefer Makefile
  targets. Delegate full gate runs to the `scrutineer` subagent.
- Commit each milestone only after its gates pass. Do not amend earlier
  commits; add new ones.
- Do not mark roadmap item 5.2.3 done until all gates and reviews are clean.

## Tolerances (exception triggers)

- Scope budget: stop and escalate if the implementation needs more than
  twelve production source files changed or more than 900 net non-test lines.
  Tests, snapshots, feature files, and documentation are excluded from the
  budget.
- Port shape: stop and escalate before changing `RouteQueue`, existing
  `JobDispatchError` variants, or any 5.2.2 job struct field.
- Dependencies: the exact Apalis pins (D10) and `googletest`
  (dev-dependency) are pre-authorized. Stop and escalate for anything else,
  including any temptation to add the `apalis` facade crate.
- Upstream surprises: if AXIOM-PIN, AXIOM-DURABLE, or AXIOM-ACK fails its
  probe or canary, or the Apalis rc APIs otherwise differ materially from
  "Context and orientation" (for example, `TaskId` or `WorkerContext` cannot be
  extracted in a handler), stop. Record the discovery and escalate with options
  rather than changing pins or reaching for the retry layer.
- Iterations: if a red test still fails after three green attempts, or a
  gate fails three consecutive runs on the same cause, stop and escalate.
- Ambiguity: if retry semantics interact with idempotency in a way not
  settled here, stop and present options.

## Risks

- Risk: release-candidate APIs drift. Severity: medium. Likelihood: low
  after D10. Mitigation: exact pins (AXIOM-PIN), the V8 canaries, and issue
  #550 for the deliberate 1.0.0 move. Apalis 1.0.0's `get_jobs` still filters on
  `run_at`, so D11 is expected to carry over; #550 re-verifies it.
- Risk: a worker crash between the tracker's increment and the ack loses
  that one attempt's count. A handler that crashes the whole process (for
  example, out-of-memory) is therefore never acked and can loop without
  dead-lettering once 5.3.1 configures orphan recovery. Severity: medium.
  Likelihood: low. Mitigation: per-attempt structured logging (job id, attempt
  number) survives the crash, and the EP-M4 runbook includes the crash-loop
  detection query. D11 narrows this from the earlier per-lease loss of a whole
  retry chain to a single in-flight attempt.
- Risk: a scheduling update fails (database error or lost lock). Severity:
  low. Likelihood: low. Effect: for a retryable error, the ack still records
  `Failed`, but with an unchanged, past `run_at`, so the job is redelivered on
  the next poll without backoff. It remains bounded by `max_attempts`. For a
  fatal error, the row stays re-fetchable and the handler fails fatally again
  on redelivery, still bounded by `max_attempts`. Mitigation: `warn`-level log
  plus a `schedule_failure` label on the retry counter. Retry no further; do
  not escalate the failure into a panic.
- Risk: backoff precision is bounded below by the storage poll interval,
  because failed rows are rediscovered only by polling (the notify trigger
  fires on insert only). Severity: low. Likelihood: certain. Mitigation:
  documented in ADR-002; production delays are seconds or longer, and tests
  configure a short poll interval.
- Risk: behavioural tests need test-only job consumption, which could drift
  into building 5.3.1's worker early, or diverge from what 5.3.1 builds.
  Severity: low. Likelihood: medium. Mitigation: the harness lives under
  `backend/tests/support/` and is not part of the production binary. The
  handler wrapper itself is production adapter code, so 5.3.1 reuses it rather
  than reinventing it, and the harness's worker-construction function is named
  as the reference wiring 5.3.1 must promote or consciously diverge from.
- Risk: `rstest-bdd` scenarios with embedded PostgreSQL and real polling
  could be slow or flaky. Severity: medium. Likelihood: medium. Mitigation:
  millisecond delays, a short poll interval, assertions on counts, orderings,
  and terminal states only, hard iteration and deadline caps, and reuse of
  `backend/tests/support/embedded_postgres.rs`.
- Risk: `apalis.jobs` grows without bound (`Done`, `Killed`, and dead
  `Failed` rows are never pruned upstream), degrading the dead-letter listing
  over time. Severity: low now, medium later. Likelihood: high eventually.
  Mitigation: `list_dead` is ordered and limited, and the `EXPLAIN` check and
  optional partial index (D6) keep the query plan honest. Row retention is
  explicitly unowned here and proposed for roadmap section 6.

## Skills and reference documents

Skills to load when implementing: `leta`, `rust-router` (then `rust-errors` for
the error taxonomy, `rust-async-and-concurrency` for the handler wrapper, and
`rust-types-and-apis` for the newtypes), `hexagonal-architecture`, `proptest`,
`rust-unit-testing`, `rust-unused-code` (for the pre-5.3.1 public factory seam),
`nextest`, `arch-decision-records` (for the ADRs), `commit-message`,
`pr-creation`, and `en-gb-oxendict`.

Repository documents to read first:

- `docs/wildside-backend-architecture.md` — "Background Job Execution
  (Apalis Workers)" section (retry/dead-letter design intent, scope split
  between 5.2.1–5.2.4 and 5.3.1, the `decode_job` seam, and the promise of
  per-job-type queues that shapes the policy-per-adapter decision D7).
- `docs/backend-roadmap.md` — sections 5.2 and 5.3 (scope overlap with
  5.3.1's "bounded retries, and dead-letter handling").
- `docs/developers-guide.md` — "Apalis queue adapter boundaries",
  "Background job payloads", "Queue build requirements", and "Queue test
  infrastructure".
- `docs/rstest-bdd-users-guide.md` and
  `docs/pg-embed-setup-unpriv-users-guide.md` — behavioural-test and
  embedded-PostgreSQL conventions.
- `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/rust-doctest-dry-guide.md`, and
  `docs/complexity-antipatterns-and-refactoring-strategies.md` — testing and
  structure conventions.
- `docs/documentation-style-guide.md` — ADR template ("an ADR captures one
  accepted decision") and style rules.
- This plan's `Surprises & discoveries` section, entries dated 2026-10-14,
  for the evidence behind D9–D11.

## Conformance basis

Upstream artefacts (no Terms of Reference document exists for this repository;
the roadmap and architecture document are the governing artefacts):

- `docs/backend-roadmap.md` item 5.2.3 (identifier ROAD-5.2.3) and the
  section 5 preamble constraint that workers interact with the domain only via
  ports (ROAD-5-PORTS).
- `docs/wildside-backend-architecture.md`, "Background Job Execution
  (Apalis Workers)" (identifier ARCH-JOBS): bounded retries with exponential
  backoff, dead-letter after max attempts, per-job-type queue configuration,
  and the 5.2.3/5.3.1 scope split.
- `docs/execplans/backend-5-2-2-job-structs-for-generate-route-and-enrichment.md`
  scope reservations (EP-522-SCOPE): job envelope stability and deferred
  retry/dead-letter ownership.
- New ADRs produced by this plan:
  `docs/adr-002-durable-retry-scheduling-for-apalis-jobs.md` (ADR-002, the
  retry mechanism: durable `run_at` scheduling by a handler wrapper, the ban on
  in-process retry middleware at the rc pins, fatal dead-lettering by shrinking
  `max_attempts`, and the exact-pin policy) and
  `docs/adr-003-dead-letters-as-apalis-rows-behind-a-domain-port.md` (ADR-003,
  the dead-letter representation, guarded transitions, budget restoration,
  requeue provenance, and the no-automated-requeue rule). The split follows the
  style guide's one-decision-per-ADR rule.

Trace links:

```plaintext
ROAD-5.2.3 -> ARCH-JOBS -> EP-M1 -> backend/src/domain/jobs/retry.rs (policy + proptest invariants V1-V3)
ROAD-5.2.3 -> ARCH-JOBS -> EP-M2 -> handler wrapper + retry scheduling (BDD: queue_retry_policy.feature, V3b-V5, V9)
ROAD-5.2.3 -> ARCH-JOBS -> EP-M3 -> DeadLetterQueue port + PostgreSQL adapter (BDD: queue_dead_letter.feature, V6)
ROAD-5-PORTS -> EP-M1..M3 -> domain imports audit (no adapter types in domain)
EP-522-SCOPE -> EP-M2 -> insta snapshots unchanged for V1 envelopes (V7)
AXIOM-PIN/AXIOM-DURABLE/AXIOM-ACK -> EP-M0 pin + runtime probe -> EP-M2 V8 rc-canary tests
ADR-002/ADR-003 -> EP-M4 -> docs updates + contents.md registration
```

## Interfaces and dependencies

Domain layer (new module `backend/src/domain/jobs/retry.rs`, re-exported from
`backend/src/domain/jobs/mod.rs`, whose module documentation is updated to
cover policy as well as payloads). Names are indicative; refine them during
implementation without changing intent:

```rust
/// Why a job execution attempt failed, as classified by the handler.
pub enum JobErrorKind {
    /// Transient failure; retrying with backoff may succeed.
    Retryable,
    /// Permanent failure; retrying cannot succeed.
    Fatal,
}

/// Maximum UTF-8 byte length retained in a job failure message.
pub const MAX_JOB_FAILURE_MESSAGE_BYTES: usize = 1024;

/// A classified, payload-free job execution failure returned by handlers.
pub struct JobFailure {
    kind: JobErrorKind,
    message: String, // at most MAX_JOB_FAILURE_MESSAGE_BYTES, on a character boundary
}

impl JobFailure {
    /// Construct a payload-free failure, truncating at a UTF-8 character boundary.
    pub fn new(kind: JobErrorKind, message: impl Into<String>) -> Self;
    pub fn kind(&self) -> JobErrorKind;
    pub fn message(&self) -> &str;
}

/// A jitter sample in the unit interval, validated at construction.
pub struct JitterSample(f64); // JitterSample::new(x) errors unless 0.0 <= x <= 1.0

/// Upper bound on any single backoff delay (Decision D13).
pub const MAX_BACKOFF_DELAY: Duration = Duration::from_secs(60 * 60);

/// Configuration values validated when building a [`JobRetryPolicy`].
pub struct JobRetryPolicyConfig {
    pub max_attempts: NonZeroU32,
    pub base_delay: Duration,
    pub multiplier: f64,
    pub max_delay: Duration,
    pub jitter_fraction: f64,
}

/// Bounded exponential backoff policy for background jobs.
pub struct JobRetryPolicy {
    max_attempts: NonZeroU32,
    base_delay: Duration,
    multiplier: f64,       // >= 1.0, validated
    max_delay: Duration,   // <= MAX_BACKOFF_DELAY, validated
    jitter_fraction: f64,  // 0.0..=1.0, validated; delay varies +/- this fraction
}

pub enum RetryDecision {
    /// Retry no earlier than the given delay from now.
    Retry { delay: Duration },
    /// Stop retrying; the job is dead-lettered.
    DeadLetter,
}

impl JobRetryPolicy {
    /// Errors unless multiplier >= 1.0, jitter is in [0, 1], and the capped
    /// delay (including jitter) fits MAX_BACKOFF_DELAY. `max_attempts` is a
    /// `NonZeroU32`, so zero attempts cannot be configured.
    pub fn new(config: JobRetryPolicyConfig) -> Result<Self, JobRetryPolicyBuildError>;
    pub fn decide(&self, attempt: NonZeroU32, kind: JobErrorKind, jitter: JitterSample) -> RetryDecision;
    pub fn backoff_delay(&self, attempt: NonZeroU32, jitter: JitterSample) -> Duration;
    pub fn max_attempts(&self) -> NonZeroU32;
}
```

The jitter sample maps linearly onto `[-jitter_fraction, +jitter_fraction]`
around the nominal delay (0.5 is the midpoint, meaning no jitter). The policy
stays pure and deterministic under test, and the adapter supplies the
randomness. `JobRetryPolicyBuildError` follows the style of
`EnrichmentJobBuildError`. Handlers map source errors through a stated
convention, `impl From<&EnrichmentSourceError> for JobErrorKind` honouring
`is_retryable()`, so 5.3.1's handlers inherit the bridge rather than inventing
it. A `decode_job` failure maps to `JobErrorKind::Fatal`.

Domain port (new file `backend/src/domain/ports/dead_letter_queue.rs`, with its
error defined via `define_port_error!`):

```rust
/// Opaque identifier for a dead job; the storage representation is an
/// adapter detail.
pub struct DeadJobId(String);

/// Why a job is dead.
#[non_exhaustive]
pub enum DeadReason {
    /// A retryable failure reached this queue's retry-policy limit.
    Exhausted,
    /// A fatal error short-circuited retries.
    Fatal,
    /// A legacy row has no persisted reason, and its status cannot identify it.
    Unclassified,
}

/// A job parked after a terminal outcome.
#[non_exhaustive]
pub struct DeadJob {
    pub id: DeadJobId,
    pub job_type: String,
    pub reason: DeadReason,
    pub attempts: u32,
    /// Size-bounded, payload-free description of the last failure.
    pub last_error: Option<String>,
    /// Number of times an operator has requeued this job before.
    pub requeue_count: u32,
    pub dead_at: Option<DateTime<Utc>>,
}

#[async_trait]
pub trait DeadLetterQueue: Send + Sync {
    /// Newest-dead first. `limit` is a page size; pagination is an
    /// additive future evolution of this pre-1.0 internal port.
    async fn list_dead(&self, limit: u32) -> Result<Vec<DeadJob>, DeadLetterError>;
    /// Guarded: fails with `DeadLetterError::NotDead` if the row does not
    /// currently satisfy the dead predicate (absent, live, or running).
    /// Restores the queue's configured retry budget (Decision D12).
    async fn requeue(&self, id: &DeadJobId) -> Result<(), DeadLetterError>;
    /// Guarded like `requeue`; deletes the row (Decision D5).
    async fn discard(&self, id: &DeadJobId) -> Result<(), DeadLetterError>;
}
```

`DeadLetterError` variants:

- `NotDead { message }`: the guard failed, because the row is absent or not
  currently dead.
- `UnknownQueue { message }`: `requeue` found a dead row whose `job_type` has
  no configured retry budget, so it refuses to guess one.
- `Unavailable { message }`: a storage failure.

The expected eventual driver of this port is an operator or admin surface in
roadmap section 6 (and possibly 5.3.1 worker-side tooling). Until then its only
callers are behavioural tests. This is an accepted, documented gap: the 03:00
operator interface is SQL, and EP-M4's runbook provides it.

Outbound dead-letter adapter (new file
`backend/src/outbound/queue/postgres_dead_letter.rs`):
`PostgresDeadLetterQueue` holds an `sqlx::PgPool` and a `RequeueBudgets` map
from Apalis queue name (the `job_type` column) to `NonZeroU32`. It is built
from the same per-queue `JobRetryPolicy` instances as the enqueue adapters (D7,
D12).

- The dead predicate is:

  ```sql
  status = 'Killed'
  OR (status = 'Failed' AND (max_attempts IS NULL OR attempts >= max_attempts))
  ```

- For wrapper-originated terminal failures, `DeadReason` comes from the
  namespaced JSONB metadata key authorized by D15. Legacy dead rows without a
  recognized key map to `Unclassified`; this includes nullable legacy `Failed`
  rows, which Apalis cannot fetch because its SQL comparison with a null limit
  is not true. Neither `Killed` nor `Failed` alone distinguishes fatal failure
  from retry exhaustion.
- `list_dead` orders by `done_at DESC` and applies `LIMIT`.
- `requeue` reads the row's `job_type` to look up its budget (returning
  `UnknownQueue` if none is configured), then executes one guarded update. The
  update sets `status = 'Pending'`, `attempts = 0`, `max_attempts = $budget`,
  `run_at = now()`, `lock_by = NULL`, and `lock_at = NULL`, with
  `WHERE id = $1 AND job_type = $2 AND (<dead predicate>)`. Under accepted D15,
  the same guarded update atomically increments
  `metadata['wildside.retry.requeue_count']`. The counter starts at zero when
  absent, is bounded to `u32`, and preserves all other metadata keys. The
  update also removes `metadata['wildside.retry.dead_reason']` so a requeued
  row cannot expose a stale terminal reason. The terminal handler writes
  `fatal` or `exhausted` to that key before the ack. Never store either value in
  `last_result`: Apalis replaces that field on the next acknowledgement. Zero
  rows affected maps to `NotDead`.
- `discard` is a guarded `DELETE ... RETURNING`, logging the returned row
  (without payload) for forensics.

Outbound retry wiring (new files under `backend/src/outbound/queue/`, for
example `retry_schedule.rs` for the SQL and `retrying_handler.rs` for the
wrapper):

- `PostgresRetryScheduler` (holding the Apalis `PgPool`) exposes two guarded
  statements, both
  `WHERE id = $task_id AND lock_by = $worker AND status = 'Running'`:
  - `schedule_retry(task_id, worker, delay)`:
    `UPDATE apalis.jobs SET run_at = now() + $delay`.
  - `record_terminal_failure(task_id, worker, attempt, reason, shrink_limit)`:
    conditionally sets `max_attempts = $attempt` and writes the reason to
    `metadata['wildside.retry.dead_reason']` in one guarded statement under
    accepted D15.
  Each returns whether exactly one row was updated.
- The handler wrapper is a `tower::Service<PgTask<Value>>` passed to
  `WorkerBuilder::build`. A plain service is accepted through the blanket
  `impl IntoWorkerService<B, S, Args, Ctx> for S` (`apalis-core`
  `src/task_fn/mod.rs:239`). Because it receives the whole task, it reads
  `parts.attempt`, `parts.task_id`, the in-memory `parts.ctx.max_attempts()`,
  and the `WorkerContext` from `parts.data` without needing extractors. It
  decodes the payload with `decode_job` (a decode failure is `Fatal`) and calls
  the job-specific handler, which returns `Result<_, JobFailure>`. On
  `Err(failure)` it:
  1. Converts `Attempt::current()` into `NonZeroU32` (zero is a wiring bug:
     log at `error` level and treat as attempt 1).
  2. Calls `policy.decide(attempt, failure.kind, jitter)`, drawing the jitter
     from `rand`.
  3. On `Retry { delay }`: calls `schedule_retry` and records the retry
     counter.
  4. On `DeadLetter`, chooses reason `Fatal` for a fatal failure and
     `Exhausted` for a retryable failure at the policy limit. Every terminal
     failure takes the guarded terminal-update path. If
     `attempt < context max_attempts`, it shrinks the persisted limit to the
     current attempt so the ack writes an unfetchable `Failed` row; otherwise
     no shrink is needed because the ack writes `Killed`. The update also
     writes the namespaced reason key before the ack, independently of whether
     the limit needs shrinking. The wrapper records the matching metric label
     and returns the bounded plain failure message. Apalis serializes that returned
     error into `last_result` during acknowledgement, replacing its previous
     value; the library's existing JSON representation is unchanged.
  5. Emits one structured log line (job id, job type, attempt, error kind,
     decision; never the payload).
- Enqueue side: `ApalisPostgresProvider::push_job` switches from
  `push(payload)` to `push_task` with a `PgContext` built via
  `with_max_attempts(policy.max_attempts())`. One `JobRetryPolicy` is carried
  per queue-adapter instance (that is, per job family or queue), matching the
  architecture document's per-job-type queue configuration. There is no global
  constant (Decision D7).
- Metrics: dedicated `RouteQueueMetrics` methods (Decision D8):
  - `observe_retry`, labelled `scheduled` or `schedule_failure`.
  - `observe_dead_letter`, labelled `exhausted` or `fatal`.

The wrapper factory and `PostgresRetryScheduler` are public items consumed,
until 5.3.1, only by the integration-test harness. That public export is the
deliberate 5.3.1 seam (handle it per the `rust-unused-code` skill). The
harness's worker-construction function (storage + wrapper, no retry layer) is
the reference wiring 5.3.1 must promote into `backend/src/outbound/queue/` or
record why it diverged.

Dependency changes (EP-M0 and EP-M1):

```toml
# backend/Cargo.toml — exact pins (Decision D10); no other production change.
apalis-core = "=1.0.0-rc.9"
apalis-postgres = "=1.0.0-rc.8"

# [dev-dependencies]
googletest = "0.14" # resolved to 0.14.3 in Cargo.lock
```

## Verification plan

External axioms: AXIOM-PIN, AXIOM-DURABLE, and AXIOM-ACK, as defined in
"Context and orientation". In addition, embedded PostgreSQL is assumed to
behave like production PostgreSQL for the SQL used. Third-party internals are
not verified. Repository-owned logic that builds on these interfaces is
verified against the real interface (embedded PostgreSQL, real Apalis storage
and worker) at the behavioural tier, and the rc-coupling canaries (V8) turn
silent upstream drift into test failures.

Obligations:

- Obligation V1 (backoff bounded above and below): for every attempt `a`
  in `1..=max_attempts` and jitter sample `j`,
  `min(nominal(a), max_delay) * (1 - jitter_fraction) <= backoff_delay(a, j) <=
  min(nominal(a), max_delay) * (1 + jitter_fraction)`,
  where `nominal(a)` is the uncapped exponential curve. The lower bound is
  material: `>= 0` alone would be vacuous for an unsigned `Duration`. Method:
  property test (`proptest`) over generated policies, attempts, and jitter
  samples. Rationale: the invariant spans a continuous input domain that
  examples cannot cover. Artefact:
  `backend/src/domain/jobs/retry_tests/properties.rs`. Evidence:
  `cargo test -p backend retry --lib` fails before implementation (red) and
  passes after. Non-vacuity: generators emit policies across base delays from 1
  ms to minutes, multipliers 1.0–10.0, and jitter 0.0–1.0; a seeded fault
  (temporarily removing the cap clamp) must make the property fail.
- Obligation V1b (delay ceiling): construction fails whenever
  `max_delay * (1 + jitter_fraction)` exceeds `MAX_BACKOFF_DELAY`, and for
  every successfully constructed policy,
  `backoff_delay(a, j) <= MAX_BACKOFF_DELAY` for all `a` and `j`. Method:
  property test plus parameterized boundary cases just under and just over the
  ceiling. Non-vacuity: the generator must produce both accepted and rejected
  configurations (classification recorded), and weakening the constructor check
  must fail the over-ceiling cases.
- Obligation V2 (monotone growth pre-cap): with jitter fixed at the
  midpoint, `backoff_delay(a+1) >= backoff_delay(a)` until the cap is reached.
  Method: property test plus parameterized `rstest` cases at the boundary (the
  attempt where the cap first binds). Non-vacuity: generator classification
  records the fraction of cases where the cap binds, and both classes (capped
  and uncapped) must be exercised. The final 128-case run recorded 58 uncapped
  and 70 capped cases; the test asserts both counts are non-zero. As a negative
  control, inverting the multiplier failed the property, and the restored
  implementation passed it.
- Obligation V3 (decision totality and bounds): `decide` returns
  `DeadLetter` whenever `kind == Fatal` or `attempt >= max_attempts`, and
  `Retry` otherwise; no path yields a retry beyond `max_attempts`. Method:
  exhaustive parameterized tests over the finite classification
  (`Fatal`/`Retryable` × attempt below, at, and above the bound) plus a
  property test over attempt ranges, using `googletest` matchers for the
  decision shape and `pretty_assertions` for equality. Rationale: the decision
  function is a small finite state map, so exhaustive partitioning plus
  properties is proportionate. No bounded model checking (`kani`) or proof
  (`verus`) is warranted: the logic has no unsafe code, no unbounded state, and
  no lemma beyond the enumerated partitions. This is the explicit justification
  required when declining heavier rigour. Non-vacuity: each partition has a
  witness case, and a seeded off-by-one fault in the bound comparison must be
  rejected.
- Obligation V3b (wrapper decision table): the handler wrapper maps every
  combination of (classification, `Attempt::current()` relative to the policy
  bound, `Attempt::current()` relative to the in-memory context `max_attempts`)
  to exactly one action: `schedule_retry` or `record_terminal_outcome`, which
  shrinks the persisted attempt limit only when required. Successful handlers
  make no scheduling write. The wrapper records the matching retry/dead-letter
  metric label and a terminal reason (`Fatal` for fatal failures, `Exhausted`
  for retryable failures at the policy bound). Every terminal outcome persists
  that reason in the namespaced metadata key, including when the ack will write
  `Killed` because the attempt limit is already reached. Method: `rstest`
  parameterized unit tests of the wrapper's decision function (factored out as
  a pure adapter-local function), with the scheduler behind a test double.
  Rationale: an off-by-one here silently grants or steals an attempt, mislabels
  the dead reason, or leaves a fatal row re-fetchable. Non-vacuity: shifting
  either comparison by one must fail at least one case.
- Obligation V4 (lifetime retry bound with durable backoff): a job enqueued
  through the adapter carries the policy's `max_attempts` in
  `apalis.jobs.max_attempts`. A handler that always fails retryably runs exactly
  `max_attempts` times in total across storage redeliveries, after which the
  row is `Killed` with `attempts = max_attempts` and is never redelivered by
  further polling. Each redelivery begins no earlier than the `run_at` that the
  previous failure persisted (an ordering between recorded timestamps, not a
  magnitude). Method: behavioural test (`rstest-bdd`) against embedded
  PostgreSQL with the capped harness, asserting the observed run count, per-run
  `Attempt::current()` values (1, 2, …), the row's `attempts` and `status`, the
  `run_at` ordering, and that continued polling delivers nothing. It also
  verifies the persisted D15 reason is `exhausted` when a pre-existing row's
  stored `max_attempts` exceeds the current policy limit and a retryable
  failure reaches that policy limit. EP-M3's V6 checks that the dead-letter
  port maps this stored reason to `DeadReason::Exhausted`. The guarded terminal
  update shrinks the stored limit to the current attempt; the ack may store
  `Failed` because its in-memory limit is still higher, while the dead
  predicate prevents another fetch. Non-vacuity: the scenarios assert that the
  handler ran more than once (a policy that never retries fails) and that extra
  polling after death yields no runs (an unbounded loop fails). A control
  scenario with a succeeding handler must end `Done` with one attempt.
- Obligation V5 (fatal short-circuit is storage-dead): a handler returning
  a fatal failure runs exactly once. The resulting row satisfies the dead
  predicate and is never redelivered by continued polling, which is a stronger
  claim than "the harness stopped polling". The D15 metadata contains `fatal`
  even when Apalis status and attempts alone would imply exhaustion. Include a
  one-attempt policy case at the in-memory Apalis attempt limit: the ack writes
  `Killed`, no limit shrink is needed, and the metadata reports `fatal`.
  Method: behavioural test as V4 with a fatal-classified failure, including a
  post-failure polling phase and a raw metadata assertion. EP-M3's V6 verifies
  that the dead-letter listing maps that value to `DeadReason::Fatal`.
  Non-vacuity: with spare attempts, misclassifying fatal as retryable fails the
  post-failure run-count assertion; incorrect reason persistence fails the
  reason assertion. Omitting the terminal limit update when required fails the
  redelivery assertion.
- Obligation V6 (dead-letter round trip, guards, and budget restoration): a
  dead job appears in `DeadLetterQueue::list_dead` with its reason, attempt
  count, bounded last error, and requeue count. A compatibility test inserts a
  failed legacy row with a null `max_attempts` and fewer than 25 attempts into
  a disposable schema that permits nulls. It verifies that Apalis cannot fetch
  the row, while `list_dead` includes it as `Unclassified` for operator
  inspection. The hydration fallback of 25 does not apply because the fetch
  query rejects the null comparison before hydration. Unit tests cover the
  metadata reason key and verify that legacy `Killed` and `Failed` rows without
  a reason key map to `Unclassified`. `requeue` makes it run again and succeed
  (proven by observing execution, not a status flip) and restores
  `max_attempts` to the queue's configured budget. Requeue also increments its
  provenance count, and after the successful acknowledgement a job-row query
  verifies the metadata count because a `Done` row is excluded from
  `list_dead`. A requeued *fatal* job that then fails retryably must get the
  full budget again, and its dead listing must retain the requeue count across
  the later acknowledgement. Requeue clears the prior dead-reason key and the
  next terminal failure sets it again. `discard` removes the row. Requeuing a
  live or absent job returns `NotDead` and changes nothing, and requeuing a
  dead row from an unconfigured queue returns `UnknownQueue`. Listing ignores
  live jobs. Method: behavioural test (`rstest-bdd`) with embedded PostgreSQL.
  Non-vacuity: listing before any failure must be empty (a witness that the
  query filters live jobs), the guard scenarios are negative controls by
  construction, and omitting the `max_attempts` restoration fails the
  requeued-fatal scenario.
- Obligation V7 (snapshot stability): V1 job envelope snapshots from 5.2.2
  are byte-identical after this plan, and any new operator-facing rendering of
  dead jobs is snapshot-pinned with `insta`. Method: re-run the existing
  `insta` suites, and add new snapshots only for new multivariant output.
  Non-vacuity: `cargo insta test` fails on any drift by construction.
- Obligation V8 (rc-coupling canaries): the facts behind AXIOM-DURABLE and
  AXIOM-ACK hold at the pinned versions. Specifically: the `apalis.jobs`
  columns used exist; a `Failed` row with spare attempts and a past `run_at` is
  fetched; the same row with a future `run_at` is not; a `Killed` row and a
  `Failed` row with `attempts >= max_attempts` are not; the ack leaves `run_at`,
  `max_attempts`, and `metadata` untouched; and a fetched task's `Attempt`
  starts at the persisted `attempts`. Method: cheap embedded-PostgreSQL
  integration tests that promote EP-M0's probe assertions into permanent tests.
  They also assert that a terminal reason written to `metadata` survives the
  Apalis acknowledgement and that requeue clears that key while preserving all
  unrelated metadata. The ack continues to store the plain bounded error string
  in `last_result`. Rationale: issue #550 or any other pin change must fail
  tests rather than silently corrupt retry or dead-letter behaviour, and
  ADR-002 and ADR-003 list these assumptions with an "on 1.0 upgrade,
  re-verify" note. Non-vacuity: each canary asserts a positive and a negative
  case (fetchable versus not fetchable), so a vacuously green canary would
  require both sides to break in tandem.
- Obligation V9 (no in-process retry middleware): no production or test
  code installs a retry layer on an Apalis worker, and `backend/Cargo.toml` has
  no `apalis` facade dependency. Method: a conformance check recorded at each
  milestone boundary, `rg -n '\.retry\(|RetryLayer|apalis::layers' backend/`
  returning no matches and `grep -n '^apalis\s*=' backend/Cargo.toml` returning
  nothing, plus the V4 run-count assertion, which a duplicating retry layer
  would break. Non-vacuity: the check is a plain text search, so any
  reintroduction is visible to it.

If implementation reveals an invariant not listed here (for example, an
ordering constraint between requeue and concurrent fetch beyond the
guarded-update design), return to this section and extend it before continuing.

## Plan of work

### Milestone EP-M0 — pin, baseline audit, and runtime probe

The user approved D10 and D11 on 2026-10-17. Steps:

1. Run `scrutineer` for a baseline gate run on the branch as it stands, and
   record the results in `Progress`. Known issue: an orphaned postmaster can
   hold the shared embedded-PostgreSQL lock and make local `make test` time
   out; record it if it occurs, rather than chasing it. The 2026-10-19 baseline
   completed without that timeout. Its two unrelated blockers were repaired
   narrowly in the M0 candidate: `mbake validate Makefile` rejected the six-line
   `fmt` recipe, and the temporary TypeDoc fixture inherited ambient type
   discovery and failed with TS2688 for `minimatch`. The fixture now disables
   ambient `@types`; main’s `fmt-python` target keeps the `fmt` recipe within
   the validator limit.
2. Pin `apalis-core = "=1.0.0-rc.9"` and `apalis-postgres = "=1.0.0-rc.8"`
   in `backend/Cargo.toml`. These are the versions already in `Cargo.lock`, so
   no code change is expected. Run
   `cargo tree -p backend -e normal | grep -E 'apalis-(core|postgres|sql)'` and
   confirm only rc.9/rc.8/rc.9 appear (AXIOM-PIN). Update the "Queue / Apalis
   dependencies" note in `docs/developers-guide.md` with one sentence on the
   exact-pin policy and a link to issue #550. Run gates and commit.
3. Run the AXIOM-DURABLE runtime probe as a scratch integration test against
   embedded PostgreSQL. Build a `WorkerBuilder` over `PostgresStorage` with a
   short poll interval and no retry layer. Enqueue one job with
   `max_attempts = 2` and attach a handler that records each invocation's
   `Attempt::current()`. On the first invocation, before returning `Err`, the
   handler runs the guarded
   `UPDATE apalis.jobs SET run_at = now() + interval '2 seconds'` (with `id`,
   `lock_by`, and `status = 'Running'` guards). Predictions, each of which
   falsifies the axiom if it fails:
   1. The handler sees `Attempt::current() == 1` on the first run and `== 2`
      on the redelivered run.
   2. No poll fetches the row during the 2-second window (no second
      invocation within 1.5 seconds).
   3. The row is redelivered after the delay.
   4. The second failure persists `status = 'Killed'` and `attempts = 2`, and
      no third invocation occurs within a further 3 seconds.
   5. In a second job (`max_attempts = 3`), a first-run failure that runs the
      guarded `UPDATE ... SET max_attempts = 1` persists `status = 'Failed'`,
      `attempts = 1`, and `max_attempts = 1`, and the row is never redelivered.

   The probe may use wall-clock waits because it is a scratch experiment, not a
   committed test. Record the verdict, with evidence, in
   `Surprises & discoveries`, then drop the scratch test (EP-M2 re-creates the
   assertions as V8 canaries without timing magnitudes). If any prediction
   fails, stop and escalate per the upstream-surprises tolerance.

Plateau: exact pins committed, baseline recorded, and AXIOM-PIN, AXIOM-DURABLE,
and AXIOM-ACK confirmed (or escalated). Recovery: revert the pin commit, or
drop the scratch test.

### Milestone EP-M1 — domain retry policy and classification (red-green)

Red: add failing `rstest` unit tests, `googletest`/`pretty_assertions`
assertions, and `proptest` properties for `JobRetryPolicy`, `RetryDecision`,
`JobErrorKind`, `JobFailure`, and `JitterSample` (obligations V1, V1b, V2, V3).
Add `googletest` to `[dev-dependencies]`.

Green: implement `backend/src/domain/jobs/retry.rs` (pure; no adapter imports)
with constructor validation per the interface section, including the
`MAX_BACKOFF_DELAY` check. Add the
`From<&EnrichmentSourceError> for JobErrorKind` bridge, and give `JobFailure` a
message constructor capped at `MAX_JOB_FAILURE_MESSAGE_BYTES` UTF-8 bytes,
truncated only at a character boundary. Update the
`backend/src/domain/jobs/mod.rs` module documentation to cover policy as well
as payloads.

Refactor: add documentation comments with examples per house style, and keep
files under 400 lines.

Plateau: domain policy fully tested and inert. Nothing consumes it yet, which
is coherent because it is a pure library addition. Gates via `scrutineer`;
commit.

### Milestone EP-M2 — durable retry wiring, bounded attempts, terminal dead-lettering

Red: add the BDD feature `backend/tests/features/queue_retry_policy.feature`
(embedded below) and step bindings in
`backend/tests/queue_retry_policy_bdd.rs`. Add the test-only consumption
harness under `backend/tests/support/`: a worker built from `PostgresStorage`
and the production handler wrapper, with no retry layer, a short poll interval,
and hard iteration and deadline caps. Add the V8 canaries (promoting the EP-M0
probe assertions, with no timing magnitudes) and the V3b decision-table unit
tests. The scenarios fail because no wiring exists.

Green, in this order:

  1. `PostgresRetryScheduler` with the two guarded statements.
  2. The wrapper's pure decision function (V3b) and the async wrapper around it,
  including jitter sampling, payload-free structured logging, and the bounded
  plain error message returned to the ack when a job is dead.
  3. Enqueue-side `max_attempts` persistence through `push_task` with a
   `PgContext` built from the adapter's policy (D7).
  4. `observe_retry` and `observe_dead_letter` on `RouteQueueMetrics`, with
   Prometheus adapter counters, keeping `observe_enqueue` and its latency
   labels restricted to enqueue outcomes (D8).

Refactor: shape the harness's worker-construction function as the named 5.3.1
promotion candidate, and run the V9 conformance check.

Feature specification (keep it in sync with the implementation):

```gherkin
Feature: Queue retry policy with durable exponential backoff

  Scenario: A transiently failing job is retried with backoff then succeeds
    Given a queue with a retry policy of 4 attempts and millisecond backoff
    And a job whose handler fails twice then succeeds
    When the worker processes the queue
    Then the handler runs three times with attempts 1, 2 and 3
    And each rerun starts no earlier than the retry time its failure scheduled
    And the job completes with status "Done"

  Scenario: A persistently failing job exhausts its attempts and is dead-lettered
    Given a queue with a retry policy of 3 attempts and millisecond backoff
    And a job whose handler always fails with a retryable error
    When the worker processes the queue
    Then the handler runs exactly three times in total
    And the job is recorded as dead because its retries were exhausted
    And further polling delivers no more runs

  Scenario: A fatally failing job is dead-lettered immediately
    Given a queue with a retry policy of 3 attempts and millisecond backoff
    And a job whose handler fails with a fatal error
    When the worker processes the queue
    Then the handler runs once
    And the job is recorded as dead because of a fatal error
    And further polling delivers no more runs

  Scenario: Policy exhaustion keeps its reason when the stored limit is higher
    Given a pre-existing job permits 25 attempts
    And the current queue policy permits 3 attempts
    And its handler always fails with a retryable error
    When the worker processes the queue
    Then the handler runs exactly three times in total
    And the row is stored as Failed and satisfies the dead predicate
    And the persisted terminal reason is Exhausted rather than Fatal
    And further polling delivers no more runs

  Scenario: An enqueued job carries the queue's retry budget
    Given a queue with a retry policy of 5 attempts
    When a job is enqueued through the route queue adapter
    Then the stored job allows at most 5 attempts
```

Plateau: the retry policy is enforced end to end under test. Production
behaviour is unchanged except that enqueued rows now carry the bounded
`max_attempts`. Persisted-format note: this only tightens the existing column's
per-row value, and deployed rows with the default 25 remain valid; the
wrapper's terminal path also dead-letters such rows once the policy bound is
reached and stores the reason in the namespaced metadata key. On failure,
Apalis continues to serialize the returned error into `last_result`; the
wrapper supplies its bounded message, replacing the prior value without
changing Apalis's JSON representation. D15 stores the terminal reason
separately in `metadata`. Gates; commit.

### Milestone EP-M3 — dead-letter port and PostgreSQL adapter

Red: add `backend/tests/features/queue_dead_letter.feature` (embedded below)
and step bindings in `backend/tests/queue_dead_letter_bdd.rs`. Add failing unit
tests for `DeadJobId`, `DeadReason` mapping, the port error type, and the guard
semantics.

Green: define the `DeadLetterQueue` port, then implement
`PostgresDeadLetterQueue` with:

- the guarded compare-and-swap updates;
- budget restoration from `RequeueBudgets` (D12);
- durable requeue provenance and terminal reasons in namespaced JSONB
  metadata keys, which survive Apalis acknowledgements;
- discard-as-delete (D5);
- ordered and limited listing;
- a size-bounded, payload-free `last_error`.

Run the `EXPLAIN` check against a seeded table (a few thousand rows). Create
the partial index through the adapter setup path only if a sequential scan
appears (D6), and record the outcome either way.

Feature specification:

```gherkin
Feature: Dead-letter listing, requeue, and discard

  Scenario: Dead jobs are listed with their failure context
    Given a job has been dead-lettered after exhausting its retries
    When the operator lists dead jobs
    Then the listing contains the job with its reason, attempt count and last error

  Scenario: A requeued dead job runs again and can succeed
    Given a job has been dead-lettered after exhausting its retries
    And the underlying fault has been fixed
    When the operator requeues the dead job
    And the worker processes the queue
    Then the job completes with status "Done"

  Scenario: A legacy failed row with a null attempt limit is listed
    Given a disposable legacy schema permits a null max_attempts value
    And a failed row has 3 attempts, below Apalis's null fallback of 25
    When the operator lists dead jobs
    Then Apalis does not fetch the row
    And the dead listing contains it as Unclassified

  Scenario: A requeued fatal job regains its full retry budget
    Given a job has been dead-lettered by a fatal error
    And the job now fails with a retryable error
    When the operator requeues the dead job
    And the worker processes the queue
    Then the handler runs the queue's full number of attempts
    And the job is recorded as dead because its retries were exhausted

  Scenario: Requeuing a live job is rejected
    Given a pending job exists
    When the operator requeues that job
    Then the operation fails because the job is not dead
    And the job's state is unchanged

  Scenario: A discarded dead job is gone
    Given a job has been dead-lettered after exhausting its retries
    When the operator discards the dead job
    Then the listing is empty
    And further polling delivers no runs

  Scenario: Listing dead jobs ignores live jobs
    Given a pending job and a running job exist
    When the operator lists dead jobs
    Then the listing is empty
```

The successful requeue scenario also asserts that the job row retains one prior
requeue in its metadata after acknowledgement.

Plateau: dead-letter handling is usable by future operators and workers through
a domain port. Gates; commit.

### Milestone EP-M4 — documentation, ADRs, runbook, roadmap closure

Write ADR-002 and ADR-003 (named under "Conformance basis") per the style-guide
template and the `arch-decision-records` skill. ADR-002 records D10, D11, D13,
and D14, including the 2026-10-14 race evidence that ruled out in-process retry
at the rc pins. ADR-003 records D3–D6 and D12. Both list the rc-coupled
assumptions with an "on 1.0 upgrade (#550), re-verify" note naming the V8
canaries.

Update `docs/wildside-backend-architecture.md` ("Background Job Execution")
from aspiration to implemented reality, covering:

- durable `run_at` scheduling;
- lifetime attempt semantics, with the single in-flight-attempt crash
  exception;
- the fact that backoff no longer occupies worker slots;
- the poll-interval precision floor;
- the reminder that 5.3.1 must not install a retry layer at the rc pins.

Update `docs/developers-guide.md` ("Apalis queue adapter boundaries" and the
test-infrastructure sections) with the new symbols, the exact-pin convention,
and the no-retry-layer rule. Add an operator runbook section with the SQL to:

- list dead jobs;
- requeue one, restoring `max_attempts`;
- discard one;
- detect the crash-loop signature (rows repeatedly returning to
  `Pending`/`Running` with stale `attempts`).

The runbook should also name the roadmap item expected to deliver a proper
operator surface.

Register both ADRs in `docs/contents.md`. `docs/users-guide.md` is expected to
need no change, because queue internals are not client-visible; confirm this
and record it in the Decision log. Propose a roadmap note for row retention and
pruning ownership under section 6. Mark roadmap item 5.2.3 done with an
execution note. Run the full gates via `scrutineer`, including Markdown lint
and Mermaid validation; commit; and open the implementation pull request.

Plateau: feature complete, documented, and the roadmap item closed.

## Milestones and plateaus — conformance checks

At each milestone boundary, check that:

- ROAD-5-PORTS holds (no adapter imports in `backend/src/domain/`);
- the ARCH-JOBS scope split is intact (no production worker loop and no
  orphan-recovery configuration);
- V9 holds (no retry layer and no `apalis` facade dependency);
- AXIOM-PIN holds (`cargo tree` shows only the pinned Apalis versions);
- there is no unapproved dependency, public-interface, or persisted-format
  change beyond the `max_attempts` tightening, the guarded scheduling writes,
  D15's two namespaced metadata keys, and (if triggered) the D6 partial index
  named in this plan;
- the V1 envelope snapshots are unchanged;
- the trace links in `Conformance basis` are current.

Compatibility decision: no public API compatibility change is required. All
touched APIs are pre-1.0 and application-internal; no aliases, shims, or dual
implementations are permitted. Accepted D15 adds only the two namespaced values
to the existing `metadata` JSON object; it adds no schema object or separate
table. The optional additive partial index is separately pre-authorized by D6.

## Concrete steps

Work from the repository root. Representative commands (adjust test names as
they land):

```bash
git branch --show-current   # expect 5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling
cargo tree -p backend -e normal | grep -E 'apalis-(core|postgres|sql)'
cargo test -p backend retry 2>&1 | tee /tmp/test-wildside-$(git branch --show-current).out
cargo test -p backend --test queue_retry_policy_bdd 2>&1 | tee /tmp/bdd-wildside-$(git branch --show-current).out
cargo test -p backend --test queue_dead_letter_bdd 2>&1 | tee /tmp/dlq-wildside-$(git branch --show-current).out
rg -n '\.retry\(|RetryLayer|apalis::layers' backend/ ; grep -n '^apalis\s*=' backend/Cargo.toml
```

Full gates (delegated to `scrutineer` and run sequentially) are
`make check-fmt`, `make typecheck`, `make lint`, and `make test`, plus
`make markdownlint` and `make nixie` for documentation milestones.

## Validation and acceptance

Record Red-Green-Refactor evidence per milestone: the red command and its
expected failure (missing symbol or failing scenario), the green command
passing, and the refactor re-run. The BDD scenarios above are the acceptance
behaviour. Unit and property tests discharge V1–V3b, behavioural tests
discharge V4–V6 and V8, `insta` re-runs discharge V7, and the boundary
conformance check discharges V9.

Quality criteria:

- all four Makefile gates pass;
- obligations V1–V9 are discharged, with non-vacuity evidence recorded in
  `Artefacts and notes`;
- a CodeRabbit review, if requested, has no unresolved in-scope findings;
- roadmap item 5.2.3 is ticked with an execution note.

## Idempotence and recovery

Milestone changes and validation steps are additive and re-runnable. Embedded
PostgreSQL tests provision throwaway clusters; re-running is safe. If a
milestone fails mid-way, `git status` and the committed plateau of the previous
milestone define the recovery point; revert uncommitted changes rather than
patching forward. The only destructive operation introduced is `discard` (a
guarded delete of a dead row), which exists solely behind the port and its
tests. The scheduling writes (`run_at`, `max_attempts`) are guarded to the row
the worker holds. Shrinking `max_attempts` is idempotent for a given attempt;
`run_at = now() + $delay` is not, because each execution advances the deadline.
Issue that relative scheduling update once and do not retry it after an
ambiguous database response. If a future design retries scheduling writes,
compute one absolute deadline and reuse it, or add a guard that proves the
attempt has not already been scheduled. The scheduling-error recovery behaviour
is recorded under Risks. The EP-M0 pin change is reverted by restoring the two
caret requirements.

## Progress

- [x] (2026-08-16) Reconnaissance completed: code recon, documentation
  recon, and Apalis 1.0-rc capability research recorded in this plan.
- [x] (2026-08-16) Plan drafted.
- [x] (2026-08-16) Expert-panel design review completed (structural,
  contract, scaling, operational, alternative-futures, and viability lenses);
  all findings folded into this revision.
- [x] (2026-10-13) User explicitly authorized execution by invoking
  `/sdlc-implementation` for this plan.
- [x] (2026-10-13) EP-M0 paused: a Firecrawl source inspection reported
  that AXIOM-ATTEMPT was false and proposed a policy-owned increment.
- [x] (2026-10-14) Planning review of the 2026-10-13 finding: the
  policy-owned increment is rejected (D9); D10 (exact Apalis pins) and D11
  (durable `run_at` backoff) are proposed.
- [x] (2026-10-17) The user accepted D10 and D11. The plan was rewritten
  around durable scheduling (see `Read this first`), and issue
  [#550](https://github.com/leynos/wildside/issues/550) was opened for the
  Apalis 1.0.0 upgrade.
- [x] (2026-10-19) Baseline `scrutineer` gates completed at `dfe1b55`:
  `check-fmt`, `typecheck`, `markdownlint`, `nixie`, and `spelling` passed;
  `lint` stopped at the `fmt` recipe line-count check; `test` passed 1,788 Rust
  tests (four skipped), the trybuild case, and frontend tests, then had 21
  workflow-contract failures caused by the same TypeDoc TS2688 `minimatch`
  ambient-type error (585 passed, one skipped). Baseline logs are under
  `/tmp/check-fmt-wildside-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  `/tmp/lint-wildside-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  `/tmp/typecheck-wildside-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  `/tmp/test-wildside-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  `/tmp/markdownlint-wildside-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  `/tmp/nixie-wildside-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  and
  `/tmp/spelling-wildside-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`.
- [x] (2026-10-19) The corrected M0 candidate passed all seven full gates:
  `check-fmt` (2s), `lint` (221s), `typecheck` (7s), `test` (489s),
  `markdownlint` (20s), `nixie` (1s), and `spelling` (12s). `make test`
  included 1,788 passing Rust tests (four skipped), trybuild, frontend tests,
  606 workflow-contract tests (one skipped), and 115 Python tests (13 skipped).
  Logs use the `-3.out` suffix under `/tmp` with the same branch-specific names
  as the baseline logs above. The fixture isolation fix and Makefile recipe
  adjustment resolve the two baseline gate failures. The exact pins resolve
  only Apalis core rc.9, postgres rc.8, and SQL rc.9 as required by AXIOM-PIN.
  The runtime probe remains.
- [x] (2026-10-19) The initial M0 runtime probe was inconclusive: the scratch
  test failed before invoking its handler on two runs because embedded
  PostgreSQL could not connect to its admin database. The cause was diagnosed
  on 2026-10-20 and the five predictions were exercised successfully on
  2026-10-23; see the later entries below.
- [x] (2026-10-20) Diagnosed the probe setup failure with a controlled password
  comparison: missing `PG_PASSWORD` produces SQLSTATE `28P01` and the original
  five-attempt provisioning failure; the Makefile default makes both admin
  connection and template provisioning succeed. At that point the five Apalis
  predictions still required a fresh probe run with the documented environment.
- [x] (2026-10-23) Prevent recurrence of the provisioning launch error with
  non-forcing Cargo PostgreSQL defaults and regression coverage. Both new
  environment-policy cases failed before the fix because Cargo supplied no
  defaults (`/tmp/wildside-pg-defaults-red.out`). After adding the defaults, a
  direct `cargo test -p backend --test route_queue_apalis_bdd` run passed all
  14 cases with `PG_PASSWORD` and `POSTGRESQL_RELEASES_URL` removed from the
  parent environment (`/tmp/wildside-pg-defaults-runtime.out`). `force = false`
  preserves explicit caller settings. All eight candidate gates passed:
  `make fmt`, `check-fmt`, `lint`, `typecheck`, `test`, `markdownlint`,
  `nixie`, and `spelling`; logs use the `/tmp/wildside-pg-defaults-*` prefix.
  The five AXIOM-DURABLE predictions remained untested at this checkpoint; the
  later runtime result is recorded below.
- [x] (2026-10-23) Rebased the branch onto `origin/main` at
  `380e7e8ec87892a34ab9a37c499762e83e95cf48`. Resolved the Makefile and
  TypeDoc-fixture conflict by keeping main's dedicated `fmt-python` target and
  its `types: []` fixture rationale while retaining the exact Apalis pins and
  issue #550 documentation. The rebased candidate passed `make check-fmt`,
  `make test` (1,851 passed, four skipped, including compile-fail, frontend,
  workflow-contract, and Python suites), `make typecheck`, and `make lint`.
  Logs: `/tmp/wildside-post-rebase-check-fmt-20261023.out`,
  `/tmp/wildside-post-rebase-test-20261023.out`,
  `/tmp/wildside-post-rebase-typecheck-20261023.out`, and
  `/tmp/wildside-post-rebase-lint-20261023.out`.
- [x] (2026-10-23) The five-part AXIOM-DURABLE scratch probe passed against
  embedded PostgreSQL and the pinned Apalis worker/storage. It observed attempt
  values 1 and 2; no retry within 1.5 seconds and redelivery after the full
  two-second `run_at` delay; `Killed`, `attempts = 2`, `max_attempts = 2` after
  the second failure with no third invocation for three seconds; and, for a
  second job, `Failed`, `attempts = 1`, `max_attempts = 1` after shrinking a
  three-attempt budget, with no redelivery for three seconds. The temporary
  test source was removed after the run. Evidence:
  `/tmp/axiom-durable-probe-wildside-20261023-final-2.out`.
- [x] (2026-10-23) Confirmed AXIOM-PIN with `cargo tree -p backend -e normal`:
  core rc.9, postgres rc.8, and SQL rc.9 are the only Apalis releases.
  Confirmed AXIOM-ACK against the same pinned source: `ack.sql` updates only
  `status`, `attempts`, `last_result`, and `done_at`, guarded by both `id` and
  `lock_by`; `calculate_status` derives `Done`, `Failed`, or `Killed` from the
  result and the in-memory attempt and max-attempt values. The runtime probe
  observed the resulting persisted states. Tree evidence:
  `/tmp/wildside-epm0-apalis-tree-20261023.out`.
- [x] (2026-10-23) Final M0 candidate passed all seven gates sequentially:
  `check-fmt`, `lint`, `typecheck`, `test`, `markdownlint`, `nixie`, and
  `spelling`. `make test` passed 1,851 Nextest cases (four skipped), both
  compile-fail suites, frontend tests, workflow contracts (606 passed, one
  skipped), and Python tests. Logs are under
  `/tmp/wildside-epm0-*-20261023-1.out`. EP-M0's baseline, exact pins, and all
  three axioms are confirmed. The required M0 CLI review is complete, but its
  valid provenance finding exposed a proposed storage deviation; EP-M1 remains
  paused while that decision awaits the user's approval.
- [x] (2026-10-23) The local CodeRabbit CLI review completed at version
  0.9.1 against `origin/main` (`380e7e8`) through `bff8ada`, covering all seven
  changed paths. The five findings comprised one style issue, one stale date
  concern, and three substantive findings (including two duplicates). The date
  concern compared its UTC date (2026-10-22) with this plan's Europe/Berlin
  date (2026-10-23); the M0 gates completed by 01:03+02 and the review began at
  01:08+02. The `list_dead` scenario could not observe a `Done` row and now
  checks the row's metadata after acknowledgement; two duplicate findings
  correctly identified that Apalis overwrites `last_result` on acknowledgement.
  A namespaced JSONB metadata counter survives that ack, but writing `metadata`
  exceeds the approved column allow-list. Proposed D15 records that deviation,
  and the plan is `BLOCKED` pending the user's decision. The second-person
  style finding is also corrected. Review evidence:
  `/tmp/wildside-epm0-coderabbit-20261023-JRwEDL/review.out`.
- [x] (2026-10-23) The D15 reassessment completed at CodeRabbit CLI 0.9.1
  against `origin/main` (`380e7e8`) through `096c679`, reviewing all seven
  changed paths. It found that the Apalis-version paragraph still described the
  historical caret requirements; that text now distinguishes the exact current
  pins from the earlier requirements. Its date concern has the same timezone
  cause: the review ran at 01:25–01:28+02 on 2026-10-23, while the UTC date was
  still 2026-10-22; the reviewed commit predates it at 01:23+02. Review
  evidence: `/tmp/wildside-epm0-d15-reassessment-p6Rhb8/review.out`.
- [x] (2026-10-23) The final status-only ExecPlan update was formatted with
  `make fmt` and passed `check-fmt`, `markdownlint`, `nixie`, and `spelling`.
  Logs: `/tmp/wildside-epm0-plan-format-20261023.out` and
  `/tmp/wildside-epm0-finaldoc-{check-fmt,markdownlint,nixie,spelling}.out`.
- [x] (2026-10-23 Europe/Berlin) The final M0 CodeRabbit review at CLI 0.9.1
  covered the committed range from `origin/main` (`380e7e8`) through `2be12e4`
  and all seven changed paths. Its major finding was valid: a retryable failure
  at a lower current policy bound can become an unfetchable `Failed` row and
  must not be reported as `Fatal`. The proposed D15 metadata key now preserves
  that reason alongside the requeue counter; V4–V6 and V8 specify the related
  assertions if approved. The date finding is a UTC/local date boundary: the
  review ran 01:32–01:37+02 on October 23, while its UTC date was October 22,
  after commit `2be12e4` at 01:31+02. Review evidence:
  `/tmp/wildside-epm0-d15-final-review-a3rPku/review.out`. EP-M1 remains
  blocked pending approval of the two D15 metadata writes. The raw
  `last_result` format remains unchanged.
- [x] EP-M0 baseline gates, exact pins (AXIOM-PIN), AXIOM-DURABLE runtime
  probe, and AXIOM-ACK source contract.
- [x] (2026-10-25 Europe/Berlin) The user accepted D15's recommendation,
  authorizing guarded writes to the two namespaced Apalis `metadata` keys for
  bounded requeue provenance and terminal failure reason. This resolves the
  approval gate; EP-M1 is now active.
- [x] (2026-10-26) EP-M2 implementation completed. The red integration
  test first failed on the missing retry APIs
  (`/tmp/test-wildside-epm2-red-5.out`). The provider now persists each queue's
  attempt cap, the wrapper makes the V3b decision and guarded PostgreSQL
  updates, and bounded retry/dead-letter metrics are wired. The focused BDD run
  passed all six scenarios (19 tests including support linkage), the
  decision-table run passed nine cases, and the all-features Prometheus counter
  test passed. BDD assertions cover stored attempt/run time after
  acknowledgement, unrelated JSONB metadata survival, and metric outcomes. The
  first run queued behind a long-lived PostgreSQL process holding the shared
  test lock; no external process was disturbed. The focused run passed using an
  owned `/var/tmp` data root. At that checkpoint, full gates and V9 conformance
  remained pending; later V9 and gate results are recorded below. The first
  frozen full gate run passed six gates; Clippy alone found an unnecessary
  suppression and excessive positional arguments in the wrapper and scheduler.
  The candidate now groups retry inputs, queue dependencies, failure context,
  and terminal update values in named structs. The focused retry decision tests
  passed (9), and the database-backed BDD target passed (19, including
  test-support linkage). The second full sequence passed six gates, but Clippy
  found five test-only issues: excessive nesting in the decision oracle and BDD
  handler, missing `Default` on the BDD world, and long rstest/worker argument
  lists. The final focused run passes nine decision tests and all 19 BDD tests.
  The probe handler now lives in its own module, named case and worker
  configuration structs replace positional test inputs, and the BDD world
  implements `Default`. V9 and the exact Apalis pins were rechecked
  successfully. The third full sequence passed six gates; Whitaker alone
  required inner module documentation in the new configuration and decision
  modules. Those comments were added, then the following full sequence passed
  `check-fmt`, `typecheck`, `test` (1,930 passed, four skipped; both
  compile-fail checks, 614 workflow contracts with one skipped, and 115 script
  doctests with 13 skipped), `markdownlint`, `nixie`, and `spelling`. `lint`
  found three more missing inner docs in the retry BDD fixture module, its
  handler module, and the retry wrapper's unit-test module. The disk has been
  recovered; no space or PostgreSQL provisioning failures occurred. Added the
  three inner module comments. The final4 sequential gate run passed all seven
  gates on the unchanged implementation candidate: `check-fmt` (2s), `lint`
  (289s), `typecheck` (1s), `test` (443s), `markdownlint` (16s), `nixie` (4s),
  and `spelling` (12s). Tests passed 1,930 Nextest cases (four skipped), both
  compile-fail checks, 614 workflow contracts (one skipped), 115 script
  doctests (13 skipped), and all 14 lint-action checks. The embedded PostgreSQL
  retry scenarios passed. Final4 logs use the `/tmp/wildside-epm2-final4-`
  prefix. The post-run documentation gates passed, and the milestone was
  committed as `c0a66b6`. CodeRabbit then recommended splitting the failure
  orchestration helpers and time-qualifying an earlier pending-status sentence;
  the first follow-up full run found a missing `Duration` import in the new
  scheduler helper (`make lint` and `make test` failed during compilation;
  `typecheck`, formatting, and the documentation gates passed). The import has
  been added. The next run passed six gates, including all 1,930 Nextest cases,
  both compile-fail checks, the workflow contracts, and the script doctests;
  Clippy found that the terminal helper still accepted five parameters. Grouped
  its related terminal fields in `TerminalRetryPlan`. The final7 full
  sequential run passed all seven gates: `check-fmt`, `lint`, `typecheck`,
  `test` (1,930 passed, four skipped; both compile-fail checks, 614 workflow
  contracts with one skipped, 115 script doctests with 13 skipped, 14
  lint-action tests, and three frontend Node tests), `markdownlint`, `nixie`,
  and `spelling`. The refactor and its plan record are ready for the follow-up
  commit; a second CodeRabbit pass will verify its two finding resolutions.
  Evidence is in
  `/tmp/wildside-epm2-final7-lint-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`
  and the corresponding `-final7-` gate logs. Earlier evidence is in
  `/tmp/wildside-epm2-final4-lint-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`
  and the corresponding `-final4-` gate logs; final3 logs are preserved under
  the corresponding `-final3-` prefix.
- [x] (2026-10-25 Europe/Berlin) EP-M1 red test captured: the focused test
  command failed to compile on the intentionally absent retry API (`E0432` for
  both the unit and property modules), as expected. Cargo added `googletest`
  0.14.3 to `Cargo.lock` and resolved compatible dependencies. Log:
  `/tmp/test-393f0df6-4a19-41c6-a76a-35fb9e814fa0-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`.
- [x] (2026-10-25 Europe/Berlin) Implement the pure EP-M1 policy and
  classification API, including the 1,024-byte UTF-8 failure bound. The refined
  focused suite passed 50 tests, including the property tests, with 824
  filtered out. Evidence:
  `/tmp/test-393f0df6-4a19-41c6-a76a-35fb9e814fa0-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling-green-2.out`.
- [x] (2026-10-25 Europe/Berlin) Run the EP-M1 negative controls. Removing the
  configured delay clamp failed V1 on the saved cap-shrink witness; restoring
  it passed that property. Inverting the multiplier failed V2; restoring it
  passed with both saved property seeds. Changing `attempt >= max_attempts` to
  `>` failed the retryable-at-bound table case, and weakening the constructor's
  jittered ceiling failed the above-ceiling case. The restored decision table
  passed six cases and the restored constructor boundary passed three. Logs:
  `/tmp/test-393f0df6-4a19-41c6-a76a-35fb9e814fa0-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling-fault-cap-clamp.out`,
  `/tmp/test-393f0df6-4a19-41c6-a76a-35fb9e814fa0-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling-cap-restored.out`,
  `/tmp/test-393f0df6-4a19-41c6-a76a-35fb9e814fa0-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling-fault-inverted-multiplier.out`,
  `/tmp/test-393f0df6-4a19-41c6-a76a-35fb9e814fa0-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling-multiplier-restored.out`,
  `/tmp/test-393f0df6-4a19-41c6-a76a-35fb9e814fa0-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling-fault-attempt-bound.out`,
  `/tmp/epm1-attempt-bound-restored.out`,
  `/tmp/epm1-fault-constructor-ceiling.out`, and
  `/tmp/epm1-constructor-ceiling-restored.out`.
- [x] (2026-10-25 Europe/Berlin) The focused EP-M1 suite passed 51 tests with
  824 filtered out. A 128-case generated midpoint run included 58 uncapped and
  70 capped curves; a later run included 53 uncapped and 75 capped curves. The
  test asserts both classes occur rather than fixing the random distribution. A
  first attempt to add this classification check failed to compile with E0689
  because the generated multiplier needed an explicit `f64` type; after that
  annotation the focused suite passed. Evidence:
  `/tmp/epm1-focused-generated-coverage-typefix.out`.
- [x] (2026-10-25 Europe/Berlin) The first full-gate attempt passed
  `check-fmt` and `typecheck`, but `lint` identified that the five-parameter
  constructor exceeded the repository's four-parameter Clippy limit. A named
  `JobRetryPolicyConfig` value now keeps settings explicit at call sites and
  reduces the constructor to one argument (D17). The started test gate stopped
  before compilation because the gate runner's `PATH` omitted the installed
  `pg_worker`; the next run explicitly supplies the shared Cargo bin directory.
  Logs: `/tmp/epm1-final-lint.out`, `/tmp/epm1-final-typecheck.out`, and
  `/tmp/epm1-final-test.out`.
- [x] (2026-10-25 Europe/Berlin) After changing the constructor to accept
  `JobRetryPolicyConfig`, the focused retry suite passed again: 51 passed, 824
  filtered out. This run recorded 53 uncapped and 75 capped generated curves.
  Evidence: `/tmp/epm1-config-focused.out`.
- [x] (2026-10-25 Europe/Berlin) The next full-gate attempt passed
  `check-fmt` and stopped at `lint`: Clippy also flags the parameterized
  invalid-configuration test's five separate inputs. The test now accepts one
  `JobRetryPolicyConfig` plus the expected error, with a four-parameter helper
  constructing named configurations. This keeps the test readable without a
  lint suppression. The focused suite then passed all 51 tests (824 filtered;
  this run generated 66 uncapped and 62 capped curves). Evidence:
  `/tmp/epm1-final-env-lint.out` and `/tmp/epm1-test-arity-focused.out`.
- [x] (2026-10-25 Europe/Berlin) The next sequential gate run passed
  `check-fmt`, `lint`, and `typecheck`, then `make test` reached 1,017 of 1,899
  Nextest cases before interruption. Database-backed tests repeatedly failed at
  setup with `invalid value for parameter "TimeZone": "UTC"`. Inspection found
  that wildcard resolution selected embedded PostgreSQL 18.6.1, whose installed
  timezone directory contains `UCT` but no `UTC`; SQLx 0.8.6 sends
  `TimeZone=UTC` during connection startup. The repository's cache warm-up
  already pins 16.10.0, but direct Cargo test processes had no `PG_VERSION_REQ`
  default. `.cargo/config.toml` now supplies non-forcing `PG_VERSION_REQ` at
  `=16.10.0`, and the environment policy contract guards that default. The
  interrupted log was briefly shared by a duplicate gate invocation and is
  treated only as failure evidence. Evidence: `/tmp/epm1-arity-check-fmt.out`,
  `/tmp/epm1-arity-lint.out`, `/tmp/epm1-arity-typecheck.out`, and
  `/tmp/epm1-arity-test.out`.
- [x] (2026-10-25 Europe/Berlin) The PostgreSQL version-default contract passed
  all seven tests, including the non-forcing `PG_VERSION_REQ` case. The first
  focused Apalis run waited on the shared cluster lock retained by the orphaned
  PostgreSQL 18 process from the interrupted full suite. A retry with isolated
  `TMPDIR` and `PG_DATA_DIR` then exposed a missing `target/pg_worker`; after
  `make prepare-pg-worker`, the fresh-cluster Apalis BDD target passed all 14
  tests, including SQLx connection startup with `TimeZone=UTC`. The orphaned
  server was left untouched. Logs:
  `/tmp/test-epm1-environment-policy-contract.out`,
  `/tmp/test-epm1-pg16-apalis-bdd.out`,
  `/tmp/test-epm1-pg16-apalis-bdd-isolated-lock.out`,
  `/tmp/prepare-pg-worker-epm1.out`, and
  `/tmp/test-epm1-pg16-apalis-bdd-ready.out`.
- [x] (2026-10-25 Europe/Berlin) Rebase the branch onto the refreshed PR target
  `origin/main` at `e7ab91e`, preserving the exact 25-commit series from its
  verified base `380e7e8`. The PR base SHA and the first branch commit's parent
  both identified that base; all commits were single-parent and the range
  contained no merges. The rebase used Git's built-in `zdiff3` machinery
  because Weave was selected only by the user-wide attributes file. It
  completed without conflicts, and `git range-diff` matched all 25 old/new
  commits. The staged EP-M1 candidate was saved and restored with its index.
  The current target's `Cargo.lock` was used as the starting point, then
  `cargo update --workspace` rebuilt the lock for `googletest` 0.14.3 and its
  compatible dependency graph. The developers' guide retains both upstream
  runner guidance and the PostgreSQL runner contract. Rebase receipt and
  recovery materials are under `/tmp/rebase-epm1-20261025-*`.
- [x] (2026-10-25 Europe/Berlin) The complete EP-M1 deterministic gate sequence
  passed on staged candidate tree `2be4cfa` at rebased `HEAD` `1811f516`. The
  test gate passed 1,900 Nextest cases (four skipped), both compile-fail
  checks, 614 workflow-contract tests (one skipped), 115 script doctests (13
  skipped), 14 action tests, and frontend tests. `typecheck`, `lint`, Markdown
  lint, Mermaid validation, and spelling also passed. Lint emitted only
  non-fatal existing Dependabot YAML and OpenAPI unused-schema warnings.
  Evidence: `/tmp/epm1-rebased-check-fmt.out`, `/tmp/epm1-rebased-test.out`,
  `/tmp/epm1-rebased-typecheck.out`, `/tmp/epm1-rebased-lint.out`,
  `/tmp/epm1-rebased-markdownlint.out`, `/tmp/epm1-rebased-nixie.out`, and
  `/tmp/epm1-rebased-spelling.out`.
- [x] (2026-10-25 Europe/Berlin) The first EP-M1 CodeRabbit CLI review
  completed on commit `17e3ea8` against `origin/main` at `e7ab91e`. It reported
  three overlapping trivial/minor style findings: tests and doctests used
  `.unwrap()`, and three plan passages used first-person possessives. The
  findings are addressed in the current candidate; the duplicate optional
  diagnostic finding is covered by the same `.expect(...)` edits. Evidence:
  `/tmp/coderabbit-393f0df6-4a19-41c6-a76a-35fb9e814fa0--5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling-.out`.
- [x] (2026-10-25 Europe/Berlin) CodeRabbit reassessed commit `3bcf1b0`
  against `origin/main` at `e7ab91e` and identified two minor issues: the plan
  overstated relative `run_at` idempotence, and Cargo's global environment
  exposed the test password to non-test commands. Both are corrected in the
  current candidate: the plan prohibits blind retries after ambiguous writes;
  Cargo no longer supplies the password globally, and the test recipe provides
  it only to its Rust test processes. All seven full gates passed on the fix6
  candidate; CodeRabbit reassessment remains pending. Evidence:
  `/tmp/coderabbit-393f0df6-4a19-41c6-a76a-35fb9e814fa0-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`.
- [x] (2026-10-26 Europe/Berlin) All seven sequential gates passed on the
  frozen fix6 candidate: check-fmt, test, typecheck, lint, markdownlint, nixie,
  and spelling. The tree SHA was `7c8e701`; the diff SHA was
  `1507c4d30763cd6c3df0ea71287531757358da3cf67917268ee7c618d6b160c1`. Evidence:
  `/tmp/epm1-review-fix6-check-fmt.out`, `/tmp/epm1-review-fix6-test.out`,
  `/tmp/epm1-review-fix6-typecheck.out`, `/tmp/epm1-review-fix6-lint.out`,
  `/tmp/epm1-review-fix6-markdownlint.out`, `/tmp/epm1-review-fix6-nixie.out`,
  and `/tmp/epm1-review-fix6-spelling.out`.
- [x] (2026-10-26 Europe/Berlin) CodeRabbit CLI 0.9.1 reviewed `fe1ca1d`
  against `origin/main` and found four valid issues: clarify the uncapped
  `base_delay`, scope the Make export assertion to the `test-rust` recipe,
  recognize `=` as an export-name boundary, and use the implemented
  `backoff_delay` API in D11. The current stop-hook run then failed because the
  filesystem ran out of space while Cargo wrote a rustdoc fingerprint and Nixie
  created its temporary directory. The user confirmed that disk capacity has
  since been recovered; rerun all gates on the corrected candidate.
- [x] (2026-10-26 Europe/Berlin) The first attempt to add a synthetic Make
  export regression exposed an `E0061` helper-signature mismatch during
  `make test`; the helper now accepts the Makefile source used by both the real
  contract and synthetic boundary cases. `check-fmt` passed before the compile
  failure. Evidence: `/tmp/epm1-coderabbit-fix1-check-fmt.out` and
  `/tmp/epm1-coderabbit-fix1-test.out`. The full sequence must be rerun.
- [x] (2026-10-26 Europe/Berlin) Fixed the four CodeRabbit findings on the
  current candidate: clarified the uncapped `base_delay`, scoped the Make
  export contract to the `test-rust` recipe, recognized `=` as an export-name
  boundary, and corrected D11 to call `backoff_delay`. All seven sequential
  gates then passed: `check-fmt` (3s), `test` (610s), `typecheck` (7s), `lint`
  (233s), `markdownlint` (28s), `nixie` (5s), and `spelling` (18s). The test
  run used `/var/tmp/wildside-epm1-full-igo1j3xy` for isolated PostgreSQL
  state; it passed 1,901 Nextest cases (four skipped), both trybuild checks,
  614 workflow doctests (one skipped), 115 script doctests (13 skipped), and
  the frontend and lint-actions suites. Candidate HEAD/tree/diff were
  `fe1ca1dcfdc65677c6acd9e92f052c4fc4e17333`,
  `cb016e25228e20b8d4e9c77df7ed33f7ab93d36c`, and
  `92fb496de77d660549f3e37826bf97629f438136031525d74c36ea1a86b2bbe6`. Evidence:
  `/tmp/epm1-full2-check-fmt-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  `/tmp/epm1-full2-test-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  `/tmp/epm1-full2-typecheck-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  `/tmp/epm1-full2-lint-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  `/tmp/epm1-full2-markdownlint-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  `/tmp/epm1-full2-nixie-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  and
  `/tmp/epm1-full2-spelling-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`.
- [x] (2026-10-26 Europe/Berlin) Committed the gated M1 candidate as `1c329be`
  and completed CodeRabbit CLI 0.9.1 review against `origin/main` at `e7ab91e`.
  The committed review covered all 14 paths and reported the same
  Makefile-contract gap twice: a bare `export` directive globally exports all
  Make variables, but the helper only recognized named exports. The current
  Makefile has no bare directive; this is a missing regression guard. Review
  log: `/tmp/coderabbit-epm1-1c329be-3ShJIM-review.out`.
- [x] (2026-10-26 Europe/Berlin) Added detection and synthetic cases for bare
  global `export`, including a trailing comment and named exports that list the
  sensitive variable after another name. The focused test failed against the
  previous helper exactly at the bare-export assertion. Replacing the helper
  with the corrected implementation then passed. All seven sequential gates
  passed: `check-fmt` (4s), `test` (715s), `typecheck` (7s), `lint` (180s),
  `markdownlint` (26s), `nixie` (6s), and `spelling` (16s). The test run used
  `/var/tmp/wildside-epm1-cr-fix-81leewbf`; all 1,901 Nextest cases passed
  (four skipped), as did both trybuild checks, workflow and script doctests,
  frontend tests, and lint-actions tests. Candidate HEAD/tree/diff were
  `1c329be4243a7992a6f1777734de9e5d72d90839`,
  `a2313f1f3413a62f5d5f5933ce30842095725f53`, and
  `9de37767b7788940919e0c38182cd26d5a520235fdb848b327386fa9d97d0b8e`. Evidence:
  `/tmp/epm1-coderabbit-fix3-check-fmt-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  `/tmp/epm1-coderabbit-fix3-test-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  `/tmp/epm1-coderabbit-fix3-typecheck-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  `/tmp/epm1-coderabbit-fix3-lint-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  `/tmp/epm1-coderabbit-fix3-markdownlint-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  `/tmp/epm1-coderabbit-fix3-nixie-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  and
  `/tmp/epm1-coderabbit-fix3-spelling-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`.
- [x] (2026-10-26 Europe/Berlin) Committed the bare-export guard as `20c72d2`.
  The first reassessment attempt returned a 403 ownership error before review
  began. CLI health and identity checks confirmed the signed-in `leynos`
  account and organization; a fresh committed review then completed against
  `origin/main` at `e7ab91e`, covering all 14 paths. It reported two trivial
  variants of one remaining gap: the matcher did not recognize Make assignment
  operators `:=`, `::=`, `:::=`, `?=`, `+=`, and `!=`, and the synthetic cases
  should cover both protected variables. Review log:
  `/tmp/coderabbit-epm1-20c72d2-fresh-fnsZNP-review.out`.
- [x] (2026-10-26 Europe/Berlin) Expanded the synthetic cases to cover every
  supported assignment operator for both protected variables, alongside
  negative checks for longer variable names. The focused test failed against
  the previous matcher at `export PG_PASSWORD:=secret`; the matcher now checks
  explicit Make assignment operators. Red-test evidence:
  `/tmp/epm1-make-operators-red.out`.
- [x] (2026-10-26 Europe/Berlin) The expanded focused test passed. All seven
  sequential gates passed: `check-fmt` (4s), `test` (573s), `typecheck` (7s),
  `lint` (320s), `markdownlint` (22s), `nixie` (6s), and `spelling` (16s). The
  test run used `/var/tmp/wildside-epm1-opfix-ffk23reg`; it passed 1,901
  Nextest cases (four skipped), both trybuild checks, 614 workflow doctests
  (one skipped), 115 script doctests (13 skipped), and the frontend and
  lint-actions suites. Candidate HEAD/tree/diff were
  `20c72d2c201496216cea93d77d6459802c58e0fa`,
  `d69af07c9e83e3331444a8b3be5cbf87e49a3386`, and
  `6399975b6ab05ac94f82aa54ade71e6b59df05a176f4f23e5aa7e1613888a944`. Evidence:
  `/tmp/epm1-assignment-fix-check-fmt-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  `/tmp/epm1-assignment-fix-test-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  `/tmp/epm1-assignment-fix-typecheck-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  `/tmp/epm1-assignment-fix-lint-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  `/tmp/epm1-assignment-fix-markdownlint-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  `/tmp/epm1-assignment-fix-nixie-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  and
  `/tmp/epm1-assignment-fix-spelling-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`.
- [x] (2026-10-26 Europe/Berlin) Committed the gated matcher update as
  `d1f57de`. Fresh CodeRabbit CLI 0.9.1 review against `origin/main` at
  `e7ab91e` completed with zero findings and covered all 14 PR paths. Evidence:
  `/tmp/coderabbit-epm1-d1f57de-BYpnlQ-review.out`.
- [x] EP-M1 domain retry policy and classification.
- [x] EP-M2 durable retry wiring, bounded attempts, terminal dead-lettering.
- [ ] EP-M3 dead-letter port and PostgreSQL adapter.
- [ ] EP-M4 documentation, ADRs, runbook, roadmap closure.

## Surprises & discoveries

- Observation (2026-10-26 EP-M2 focused run): the shared test lock remains
  held by an older PostgreSQL process under the user systemd manager, so the
  first focused invocation waited before test setup. A run using owned `TMPDIR`
  and `PG_DATA_DIR` at `/var/tmp/wildside-epm2-393f0df6-pg` exercised the same
  embedded PostgreSQL code path and passed all six retry scenarios. This
  preserved the lock owner's process and data. The isolated root is not a Cargo
  target or cache. Logs: `/tmp/test-wildside-epm2-bdd-isolated-2.out`,
  `/tmp/test-wildside-epm2-retry-wrapper-unit.out`, and
  `/tmp/test-wildside-epm2-retry-metrics-unit-all-features.out`.

- Observation (2026-10-26, stop-hook validation): `make lint` could not write
  `target/.rustdoc_fingerprint.json`, and Nixie could not create a temporary
  directory because the filesystem was full. These infrastructure errors did
  not establish source failures. After the user reported recovery, `df` showed
  184 GB free; all seven sequential gates subsequently passed, including
  rustdoc and Nixie, with 175 GB still free after validation. The first test
  rerun had also exposed contention with an unrelated shared PostgreSQL lock;
  using an owned temporary root under `/var/tmp` let the complete suite pass
  without disturbing the lock owner.
- Observation (2026-10-26, post-hook recovery rerun): the stop hook again
  reported ENOSPC while Rustdoc wrote its target fingerprint and Nixie created
  a temporary directory. After the user reported that storage had been
  recovered, the complete sequential gate suite passed on clean HEAD
  `b0df3de0251b63a3717ea1e72cdfc8de1a16a88c` (tree
  `43510ecd4554890c722fc5cecc162e85ca0467a9`). Free space was 147 GiB at both
  the beginning and end. `make test` passed 1,901 Nextest tests (four skipped),
  both compile-fail checks, 614 workflow doctests (one skipped), 115 script
  doctests (13 skipped), and the frontend and lint-actions suites. The test
  used the owned PostgreSQL root `/var/tmp/wildside-b0df3de-thqa6ctq`; compiler
  admission remained healthy. Evidence:
  `/tmp/wildside-b0df3de-check-fmt-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  `/tmp/wildside-b0df3de-test-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  `/tmp/wildside-b0df3de-typecheck-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  `/tmp/wildside-b0df3de-lint-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  `/tmp/wildside-b0df3de-markdownlint-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  `/tmp/wildside-b0df3de-nixie-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`,
  and
  `/tmp/wildside-b0df3de-spelling-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling.out`.
- Observation (2026-10-25 Makefile validation): `mbake validate Makefile`
  rejects repeated declarations of `test-rust`, including target-specific
  variable assignments. The test-only PostgreSQL settings therefore belong in
  the single `test-rust` recipe. Its shell-quoted exports use `.ONESHELL` so
  they reach all three Cargo test commands while preserving values containing
  whitespace, quotes, dollar signs, or shell metacharacters. The environment
  contract test guards the one target declaration, both recipe exports, and the
  absence of global Make exports. One lint run found the duplicate target
  declarations. The corrected-candidate test run then found that the contract
  helper stripped trailing whitespace but not recipe indentation; 1,899 other
  Nextest cases passed and four were skipped. The helper now trims indentation
  too. The preceding candidate's complete test and typecheck gates passed. The
  next lint run found the recipe body exceeded `mbake`'s five-line limit; its
  two exports are now one shell command. The fix6 candidate passed all seven
  sequential gates; the available space remained above 200 GB. Evidence:
  `/tmp/epm1-review-fix3-lint.out`, `/tmp/epm1-review-fix4-test.out`,
  `/tmp/epm1-review-fix5-lint.out`, and the fix6 logs above.

- Observation (2026-10-25 initial Make target-scope experiment): GNU Make
  parsed a target-specific `export NAME` without an assignment as a
  prerequisite named `export`; the first candidate therefore stopped at
  `make test` before any tests or builds ran. The valid form is
  `target: export NAME := $(NAME)`. An isolated Make check verified that this
  passes the default only into the Rust test target's scope, keeps unrelated
  targets unset, and preserves a caller override. `mbake` later rejected the
  resulting repeated target declarations, so the final recipe uses the
  single-rule `.ONESHELL` approach described above. Evidence:
  `/tmp/epm1-review-fix-test.out` and `/tmp/epm1-review-fix3-lint.out`.

- Observation (2026-10-20 provisioning diagnosis): the direct Cargo launch
  omitted `PG_PASSWORD`, which Make exports for its tests. The setup helper
  repairs password state but does not export the resolved default. A minimal
  scratch diagnostic reproduced SQLSTATE `28P01` and the five-attempt admin
  failure without the variable (3.70 seconds); the same executable and worker
  succeeded with the Makefile password (1.23 seconds). The dependency's
  `connect_admin` formats its error with Display, hiding the SQLSTATE behind
  `db error`. This was a probe-launch error, not evidence against durable
  scheduling. No shared-service repair or dependency change is required.
  Evidence and experiment boundaries are recorded in
  [the debugging plan](../debugging/debugging-plan-2026-10-20-probe-provisioning.md).
  The scratch diagnostic was removed. The five Apalis predictions were still
  untested at that checkpoint; the later probe result supersedes that status.

- Observation (2026-10-19 baseline): the branch's unchanged full gate run
  found two gate-harness issues unrelated to retry implementation.
  `mbake validate Makefile` rejects `fmt` because it had six recipe lines; the
  workflow-contract TypeDoc fixtures all failed before evaluating their
  contracts because TypeScript tried to load implicit `minimatch` types. The
  temporary fixture has no ambient-type dependency, so it now sets `types: []`;
  main’s existing `fmt-python` target superseded the one-line workaround while
  preserving the line-count fix. Evidence: the baseline `lint` and `test` logs
  named above. Impact: these minimal repairs unblock a clean full gate run
  without changing runtime behaviour.

- Observation (2026-10-19 corrected M0 candidate): all seven sequential gates
  passed after the two baseline gate-harness repairs. The workflow-contract
  suite completed with 606 passed and one skipped; the full Rust, frontend, and
  Python test suites also passed. Candidate logs are the corresponding
  branch-specific `/tmp/*-3.out` files. Impact: the pins and narrowly scoped
  harness repairs are ready for the M0 commit; AXIOM-DURABLE still requires its
  runtime probe before the milestone is complete.

- Observation (2026-10-19 AXIOM-DURABLE probe): both runtime attempts failed
  before the handler ran. `provision_template_database` exhausted its five
  built-in attempts with `failed to connect to admin database: db error` from
  `pg-embed-setup-unpriv`'s `cluster/connection.rs`; the test binary's 13
  support-module tests passed in each run. Evidence:
  `/tmp/probe-wildside-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling-2.out`
  and
  `/tmp/probe-wildside-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling-3.out`.
  None of the five predictions was exercised in those two attempts. The
  missing-password cause was confirmed on 2026-10-20, and the successful
  replacement probe is recorded below. This historical setup failure does not
  count as evidence against AXIOM-DURABLE.

- Observation (2026-10-23 AXIOM-DURABLE probe): all five predictions passed
  against the pinned Apalis PostgreSQL worker. The Cargo configuration supplied
  the stable non-forcing `PG_PASSWORD` and `POSTGRESQL_RELEASES_URL` defaults;
  the test command supplied the worker path from `make test`. The handler
  recorded attempts 1 then 2, and the second invocation began at least two
  seconds after the first. The row then persisted `Killed` with `attempts = 2`
  and `max_attempts = 2`; no third invocation arrived in the following three
  seconds. A second job started with `max_attempts = 3`, changed its guarded
  running row to `max_attempts = 1`, and persisted `Failed`, `attempts = 1`,
  `max_attempts = 1`, without another invocation during the next three seconds.
  The scratch harness waited for Apalis's ack to persist after observing the
  handler event, and queried by the Apalis task ID because this schema stores
  the encoded job payload as `bytea`. Evidence:
  `/tmp/axiom-durable-probe-wildside-20261023-final-2.out`; the scratch source
  was deleted. Impact: AXIOM-DURABLE is confirmed for the pinned versions and
  EP-M1 is no longer blocked by this external assumption.

- Observation (2026-10-23 AXIOM-ACK source check): at pinned rc.8,
  `PgAck::ack` computes result status from `parts.attempt.current()` and
  `parts.ctx.max_attempts()`. `queries/task/ack.sql` writes only the result
  status, attempt count, last result, and completion timestamp, and matches
  both the task ID and worker lock. The durable probe separately observed the
  terminal rows produced by these rules. Impact: AXIOM-ACK is confirmed at the
  pinned version; the V8 canaries will preserve this contract in EP-M2.

- Observation (2026-10-23 CodeRabbit review): when an existing row's stored
  attempt limit exceeds the current queue policy, a retryable failure at the
  policy limit is made unfetchable by shrinking the stored limit, but the ack
  can persist it as `Failed` because the hydrated in-memory limit is still
  higher. Status-based mapping then labels it `Fatal`. This is distinct from a
  fatal failure at the same attempt and cannot always be recovered from status
  or attempts. Proposed D15 adds a namespaced metadata reason key alongside
  requeue count; until approved, the plan remains blocked. Evidence:
  `/tmp/wildside-epm0-d15-final-review-a3rPku/review.out`.

- Observation (2026-10-23 follow-up CodeRabbit review): V6 required a requeue
  count even though D15's JSONB writes remain unapproved. The V6 and feature
  specification now make that assertion conditional on D15. The dead-row
  predicate now includes nullable `Failed` rows: pinned rc.9's fetch query
  compares attempts directly with `max_attempts`, so a null limit prevents
  fetch before the hydrator's fallback of 25 can apply. Such legacy rows are
  listed as `Unclassified`; V6 tests one below 25 attempts. The canonical rc.8
  schema declares the column `NOT NULL`. Evidence:
  `/tmp/coderabbit-wildside-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling-2.out`.

- Observation (2026-10-23 final plan review): the prior null-limit compatibility
  predicate incorrectly relied on the hydrator's fallback of 25. Source review
  confirmed that `get_jobs` applies `attempts < max_attempts` first, so a null
  limit makes a failed row invisible to both retries and the old dead listing.
  The selected policy lists every nullable failed row as `Unclassified` and
  tests one below 25 attempts. The review also found that the wrapper's bounded
  error is serialized into `last_result` by every acknowledgement; the plan now
  states that this replaces the previous result while preserving Apalis's JSON
  representation. Evidence:

  ```plaintext
  /tmp/coderabbit-wildside-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling-7.out
  ```

- Observation (2026-10-23 final review correction): the policy-exhaustion
  feature scenario asserted an `Exhausted` reason while proposed D15 remains
  unapproved. It now expects `Exhausted` if D15 is approved and `Unclassified`
  otherwise. Evidence:

  ```plaintext
  /tmp/coderabbit-wildside-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling-8.out
  ```

- Observation (2026-10-23 final plan review): terminal metadata must be written
  even when the current attempt already reaches Apalis's limit and the ack will
  store `Killed`; only the persisted limit shrink is conditional. Otherwise, a
  fatal one-attempt job loses its `Fatal` reason. The D15 wrapper contract and
  V3b/V5 now require the reason update on every terminal outcome. Evidence:
  `/tmp/coderabbit-wildside-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling-3.out`.

- Observation (2026-10-23 follow-up plan review): a retryable failure at the
  policy limit also needs a guarded limit shrink when a pre-existing row's
  stored limit is higher than the current queue policy. D15 reason persistence
  is independent of that conditional shrink. V4/V5 now require reason-specific
  assertions only when D15 is approved and use `Unclassified` otherwise.
  Evidence:
  `/tmp/coderabbit-wildside-5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling-6.out`.

- Observation (2026-10-23 post-rebase validation): `make check-fmt`,
  `make typecheck`, `make lint`, `make markdownlint`, `make nixie`, and
  `make spelling` passed on HEAD `736c298f`. `make test` exited 1 after 1,085
  seconds because
  `backend::diesel_offline_bundle_repository::offline_repository_maps_missing_schema_to_query_error`
  failed at case 995/1,851. The captured nested output contains no underlying
  diagnostic. This candidate changes only this ExecPlan, and the same test
  passed in earlier runs, so the failure's cause remains unknown. No retry was
  made. Evidence: `/tmp/wildside-pg-defaults-last-test-20261023.out`.

- Observation: apalis-postgres 1.0.0-rc.6 does not persist abort errors as
  `Killed` (the `calculate_status` arm is commented out upstream), so a fatal
  failure acked with spare attempts becomes a re-deliverable `Failed` row.
  Evidence: published crate source for rc.6 (`ack.rs`), and the `get_jobs`
  predicate re-fetching `Failed` rows with `attempts < max_attempts`. Impact:
  fatal dead-lettering requires an explicit storage-level action by the adapter
  wiring; the dead predicate alone cannot see an early fatal failure (Decisions
  D4; obligation V5).
- Observation: the Apalis retry layer performs backoff in-process while
  the job row stays locked; `run_at` is not rescheduled on failure, and
  attempts persist only at final acknowledgement. Evidence: `ack.sql` in the
  published rc.6 crate updates only status/attempts/last_result/done_at; tower
  `Retry` awaits the backoff future within the same execution. Impact: delays
  must fit a total budget below any future orphan-recovery threshold;
  `max_attempts` is a per-lease bound; per-attempt logging is the only
  crash-surviving artefact (Decisions D2; obligations V1b, V4).
- Observation: nothing in this repository configures Apalis orphan
  recovery today. Evidence: `reenqueue_orphaned_after` appears nowhere in
  `backend/src`. Impact: until 5.3.1, a dead worker leaves its job `Running`
  indefinitely; recorded in "Context and orientation" and in ADR-002 rather
  than assumed away.
- Observation: the pinned Apalis worker tracker increments a task's shared
  `Attempt` once around the complete user-middleware future. Its retry policy
  reads `Attempt.current()` but does not increment it. A retry policy that
  relies on that count therefore cannot enforce the planned per-lease limit.
  Evidence: `AttemptOnPollFuture::poll` and `TrackerService::call` in the
  [apalis-core worker source](https://docs.rs/crate/apalis-core/1.0.0-rc.7/source/src/worker/mod.rs),
  the
  [Attempt type](https://docs.rs/crate/apalis-core/1.0.0-rc.7/source/src/task/attempt.rs),
  and `RetryPolicy::retry` in the
  [apalis retry source](https://docs.rs/crate/apalis/1.0.0-rc.7/source/src/layers/retry/mod.rs)
  (retrieved 2026-10-13). `WorkerBuilderExt::retry` installs a retry layer in
  user middleware, which the tracker wraps once. Impact: AXIOM-ATTEMPT is false
  as written; EP-M1–M4 must not proceed until a revised mechanism passes a
  bounded runtime probe and the user approves the plan revision. Superseded
  2026-10-14: the layer order is inverted. See the next entries.
- Proposed correction, not yet adopted: increment the shared `Attempt` in
  the custom retry policy immediately before it returns a retry future. Then
  verify that the next handler invocation observes the increment and that final
  acknowledgement persists the same total. This retains the facade retry layer
  while making attempt accounting explicit in the adapter policy. Rejected
  2026-10-14 (Decision D9).
- Observation (2026-10-14 review): the tracker is the innermost worker layer,
  so `Attempt` advances once per in-process attempt. Evidence: the worker's
  local `ServiceBuilder::layer` builds `Stack::new(layer, self.layer)`; tower's
  `Stack::new(inner, outer)` layers `inner` first and `outer` around it; the
  stack adds `self.middleware` first and `TrackerLayer` last, with the comment
  "pass the user defined middleware first" (identical in
  [apalis-core rc.7](https://docs.rs/crate/apalis-core/1.0.0-rc.7/source/src/worker/mod.rs)
  and the cached rc.9 source,
  [tower-layer `Stack`](https://docs.rs/tower-layer/latest/src/tower_layer/stack.rs.html)).
  Corroboration: upstream's own `basic_worker_retries` test in
  [apalis rc.7 retry](https://docs.rs/crate/apalis/1.0.0-rc.7/source/src/layers/retry/mod.rs)
  asserts that `Attempt::current()` reaches 4 under `RetryPolicy::retries(3)`,
  which is only possible if it advances per retry. Impact: the 2026-10-13
  conclusion is inverted, and a policy-owned increment would double-count.
- Observation (2026-10-14 review): the backend middleware (Ack wrapping
  LockTask) sits inside user middleware, so every failed in-process attempt is
  acked separately. Evidence: rc.8 `PostgresStorage::middleware` returns
  `Stack::new(LockTaskLayer, AcknowledgeLayer<PgAck>)`; rc.8 `ack.sql` sets
  `status`, `attempts`, `last_result`, and `done_at`, but not `run_at` or
  `lock_by`; the `get_jobs` predicate (migration `20251018165056_queue_jobs`)
  selects `Failed AND attempts < max_attempts AND run_at < now()` without
  regard to `lock_by`; `lock_by_id.sql` relocks `Pending`, `Queued`, or
  re-fetchable `Failed` rows. Impact: while the tower retry layer sleeps, the
  job is a re-fetchable `Failed` row, so a second worker can run it
  concurrently and the two can take the lock from each other. This is a
  duplicate-execution hazard that the original D2 did not foresee, and it
  invalidates in-process backoff over `PostgresStorage` at these pins.
- Observation (2026-10-14 review): `Cargo.lock` resolves
  `apalis-core 1.0.0-rc.9` and `apalis-postgres 1.0.0-rc.8`, not the rc.7/rc.6
  that the manifest and this plan name. The caret requirements also admit
  `1.0.0` final (published 2026-10-12). That release is a breaking rewrite:
  `Task<Args>` loses its `Ctx` and `IdType` parameters, results persist in
  batches through `handle_result.sql`, and backend middleware moves *outside*
  user middleware, so a retry layer there would be acked once. Evidence:
  crates.io version listing and the 1.0.0 crate sources
  (`apalis-core/src/worker/mod.rs`, `src/worker/lifecycle.rs`,
  `apalis-postgres/src/queries/handle_result.rs`). Impact: an unrelated
  `cargo update` would silently replace every axiom in this plan (Decision D10).
- Observation (2026-10-17, while folding in D10/D11): the facts D11 needs
  hold at rc.9/rc.8. Every fetch path filters `run_at < now()`: `get_jobs`,
  `lock_by_id.sql`, and `queue_by_id.sql` all do, and the notify trigger fires
  only on `INSERT`. `TaskRow::try_into_task` (`apalis-sql`) rehydrates both
  `Attempt` and the context `max_attempts` from the row. `WorkerBuilder::build`
  accepts any `Service<Task<..>>` through a blanket `IntoWorkerService` impl
  (`apalis-core` `src/task_fn/mod.rs:239`), so the wrapper can read `parts.ctx`
  directly. `PgContext::with_max_attempts` exists for the enqueue side, and
  `rand` 0.8 is already a dependency for jitter. One gap needed closing: the
  terminal path may shrink `max_attempts`, so `requeue` must restore it
  (Decision D12). Apalis 1.0.0's `get_jobs` also still filters on `run_at`, so
  D11 is expected to survive issue #550. Note, however, that 1.0.0 requires
  `sqlx` 0.9, and the backend is on 0.8. Evidence: cached rc sources under
  `~/.cargo/registry/src/` and the 1.0.0 crate tarballs.

## Decision log

- Verification refinement (2026-10-26): EP-M2 introduces the retry wrapper but
  EP-M3 introduces `DeadLetterQueue::list_dead`. The M2 feature therefore
  asserts the guarded row state and D15's persisted terminal reason directly;
  V6 verifies that the later listing maps the reason to the domain enum. This
  keeps M2 test-first without duplicating the dead-letter adapter early or
  weakening either acceptance check.

- Operational decision (2026-10-20): run the replacement scratch probe with
  the Makefile-equivalent PostgreSQL environment documented in the developer
  guide. The controlled authentication comparison resolves the setup blocker;
  at that checkpoint it did not discharge AXIOM-DURABLE or change D9–D14. EP-M1
  remained gated on the five-prediction runtime probe until the successful
  2026-10-23 result below.

- Operational decision (2026-10-23): AXIOM-DURABLE is confirmed at the exact
  Apalis pins, including the persisted delay and both terminal-state paths.
  AXIOM-PIN and AXIOM-ACK are also confirmed. The complete M0 gate sequence is
  green. The milestone CLI review identified a provenance and terminal-reason
  storage deviation, which D15 addressed. Implementation remained paused until
  the user accepted that decision on 2026-10-25.

- D1 — *Superseded on 2026-10-17 by D11 and D14.* Adopt the upstream Apalis
  tower retry machinery, with a custom `tower::retry::Policy` that delegates to
  the domain `JobRetryPolicy`. Rationale: the facade layer is the supported
  mechanism at these pins and integrates with the worker builder 5.3.1 will
  use; delegating keeps the verified domain function authoritative instead of
  maintaining a parallel spec in tower's backoff maker. Depending on tower
  directly would mean reimplementing the apalis-aware glue (request cloning,
  attempt bookkeeping) — exactly the brittle rc-coupled code this choice
  avoids. Date/author: 2026-08-16, planning agent (subject to user approval of
  this plan).
- D2 — *Superseded on 2026-10-17 by D11 and D13.* Accept in-process backoff
  under a hard total budget; defer persisted (`run_at`-based) backoff.
  Rationale: the honest constraint is that rc.6 exposes no failure-time hook
  with which to reschedule `run_at` (the operator-path SQL in D3 already writes
  to the library-managed table, so schema purity is not the argument). Building
  the hook means owning custom ack-path middleware that 5.3.1 will restructure.
  The strongest alternative — durable graphile-worker-style rescheduling — buys
  crash-durable backoff and freed worker slots at that cost; at the mandated
  small delay caps the durability delta is marginal. Revisit trigger, recorded
  in ADR-002: revisit if any production policy's worst-case total in-process
  wait approaches `TOTAL_BACKOFF_BUDGET`, or if 5.3.1 load tests observe
  worker-slot exhaustion during backoff. Date/author: 2026-08-16, planning
  agent.
- D3 — Dead letters are rows in `apalis.jobs` (`Killed`, `Failed` with a null
  `max_attempts`, or `Failed` with `attempts >= max_attempts`), surfaced
  through a domain `DeadLetterQueue` port; no separate table. Dead-letter
  surfaces never expose raw payloads (privacy stance inherited from
  `decode_job`); operators correlate via job id, job type, and the bounded
  last-error text. Rationale: matches Apalis's model and graphile-worker prior
  art; avoids schema divergence from library-managed tables; the port isolates
  the representation so a future table swap stays adapter-local. Date/author:
  2026-08-16, planning agent.
- D4 — Terminal errors are made dead at the storage level by adapter wiring, not
  by relying on Apalis abort persistence. *Mechanism settled on 2026-10-17
  under D11: before the ack, the handler wrapper shrinks the locked row's
  `max_attempts` to the current attempt when its in-memory limit has not
  already been reached, so the ack's `Failed` row satisfies
  `attempts >= max_attempts` and is never fetched. The alternatives sketched
  below (setting `Killed` directly, or driving the attempt counter) are
  rejected: the ack overwrites `status`, and inflating `attempts` would
  misreport how often the job ran.* Original rationale: the abort-to-`Killed`
  path is incomplete at rc.6, and a terminal `Failed` row with spare attempts
  would be re-delivered by the storage fetcher. The wiring must perform a
  guarded retry-limit shrink (or drive the attempt counter to exhaustion, per
  the AXIOM-ATTEMPT probe); either way the dead predicate holds and re-delivery
  is impossible. This also keeps behaviour stable if 1.0-final restores the
  abort arm. Date/author: 2026-08-16, planning agent.
- D5 — `discard` deletes the dead row (guarded, `RETURNING`, logged
  without payload). Rationale: the earlier marker-based sketch was
  self-contradictory — a discarded-but-`Killed` row would still match the dead
  predicate unless the listing grew a stringly-typed filter. Deletion is honest
  and bounds table growth; if retention of discarded jobs becomes a requirement
  it deserves its own decision, not a marker hack. Date/author: 2026-08-16,
  planning agent (panel finding).
- D6 — An additive partial index on the dead predicate is pre-authorized,
  created through the adapter setup path, applied only if EP-M3's `EXPLAIN`
  check shows a sequential scan. Rationale: `apalis.jobs` retains all history
  at these pins; an OR-predicate scan degrades with table growth. Conditional
  application keeps the footprint minimal and the evidence recorded.
  Date/author: 2026-08-16, planning agent (panel finding).
- D7 — One `JobRetryPolicy` per queue-adapter instance (per job family),
  no global constant. Rationale: the architecture document promises
  per-job-type queue configuration; per-instance policy state gives 5.3.1's
  per-queue workers a coherent shape without touching the `RouteQueue` port
  signature. Date/author: 2026-08-16, planning agent (panel finding).
- D8 — Keep `RouteQueueOutcome` and enqueue latency metrics restricted to
  enqueue operations. Add dedicated `observe_retry` and `observe_dead_letter`
  methods for the two new event counters. Rationale: `observe_enqueue` records
  an enqueue outcome and latency; using retry or dead-letter events as its
  outcome would mislabel queue processing as enqueue activity and pollute the
  enqueue latency histogram. Date/author: 2026-10-13, execution refinement.
- D9 — Reject the policy-owned `Attempt` increment. Rationale: the tracker
  is the innermost layer and already increments once per in-process attempt, so
  an extra increment in the retry policy would advance the count twice per
  retry. That halves the effective budget and persists `attempts` values that
  skip. Date/author: 2026-10-14, planning review.
- D10 (accepted by the user on 2026-10-17) — Pin Apalis exactly to the
  resolved versions: `apalis-core = "=1.0.0-rc.9"` and
  `apalis-postgres = "=1.0.0-rc.8"`. Rationale: caret requirements on
  prerelease versions admit `1.0.0` final, a breaking rewrite whose layer order
  and persistence path differ from everything this plan verifies. The upgrade
  to 1.0.0 should be its own roadmap item: it changes the enqueue adapter and
  the `Task` API, and once 1.0.0 is in place the V8 canaries are the
  re-verification checklist. Alternative: perform that upgrade first as a
  precursor, then re-plan 5.2.3 against 1.0.0. That would mean building worker
  code only once, on the final API, at the cost of delaying 5.2.3. Date/author:
  2026-10-14, planning review. The 1.0.0 upgrade is tracked as
  [#550](https://github.com/leynos/wildside/issues/550).
- D11 (accepted by the user on 2026-10-17) — Replace in-process tower
  retry with durable, storage-scheduled backoff. Supersedes D1 and D2 if
  approved. Mechanism: no retry layer is installed. A handler wrapper (the
  service passed to `WorkerBuilder::build`, so it runs inside the tracker and
  before the ack) classifies the error through `JobErrorKind`. It reads the
  current attempt from the extracted `Attempt` and the task id. Before it
  returns `Err`, it issues one guarded update while the row is still `Running`
  and locked by this worker
  (`WHERE id = $1 AND lock_by = $2 AND status = 'Running'`). For a retryable
  error below the policy limit, it sets
  `run_at = now() + JobRetryPolicy::backoff_delay(attempt, jitter)`. A fatal
  error or a retryable error at the policy limit takes the terminal path. If
  `Attempt::current() < context max_attempts`, the guarded update sets the
  persisted `max_attempts` to `Attempt::current()`, exactly the `attempts`
  value the ack will persist. This also handles a pre-existing row whose stored
  limit exceeds the current policy: the retryable failure at the policy limit
  must become unfetchable even though the in-memory limit still permits another
  attempt. When `Attempt::current() >= context max_attempts`, no limit shrink
  is needed because the ack already writes `Killed`. Under accepted D15, the
  terminal update also writes the namespaced reason key before the ack, whether
  or not a limit shrink is needed. Rows without that key remain `Unclassified`
  because status and attempt fields cannot identify the cause of legacy
  failures. The ack does not touch `run_at` or `max_attempts`; after a terminal
  shrink, the row may be `Failed` but is permanently dead under the existing
  dead predicate. Otherwise the ack writes `Killed` at the in-memory attempt
  limit. The fetch path rehydrates `Attempt` from persisted `attempts`.
  Rationale: at rc.7–rc.9 the in-process alternative is racy (see
  `Surprises & discoveries`). The durable form closes the race, because the row
  is never fetchable during the delay. It also frees worker slots during
  backoff, survives worker crashes with attempt history intact, makes
  `max_attempts` a lifetime bound rather than a per-lease bound, and removes the
  `apalis` facade dependency, AXIOM-FACADE, the `TOTAL_BACKOFF_BUDGET`
  coupling to orphan recovery, and the custom `tower::retry::Policy`. The cost
  is one extra SQL statement per failure, and backoff precision bounded below
  by the poll interval. Panel prior art (graphile-worker, sidekiq) uses this
  model. The domain `JobRetryPolicy` remains the executed policy. Consequences
  to apply before EP-M1 if approved: rewrite D1, D2, D4, the "Interfaces and
  dependencies" adapter section, obligations V1b, V4, and V8, the EP-M2 green
  step, and the Risks entries tied to in-process backoff. Replace AXIOM-ATTEMPT
  with AXIOM-DURABLE, the five-part runtime probe in EP-M0. Date/author:
  2026-10-14, planning review. These consequences were applied on 2026-10-17.
  The wrapper is a `tower::Service` over the task, so it reads the in-memory
  `max_attempts` from `parts.ctx` rather than an extractor.
- D12 — `requeue` restores the queue's configured retry budget. Rationale:
  under D4 and D11, a fatally dead row has `max_attempts` equal to its attempt
  count. Resetting only `attempts = 0` would leave a requeued job with a budget
  of one. `PostgresDeadLetterQueue` is therefore built with a `RequeueBudgets`
  map from Apalis queue name (`job_type`) to the `max_attempts` of that queue's
  `JobRetryPolicy`, and it refuses with `UnknownQueue` rather than guess. A
  budget lookup keeps the port signature operator-friendly. Keep the queue's
  original budget out of metadata: the column is Apalis-managed and its shape
  may change in 1.0.0. Accepted D15 records the distinct requeue count and
  terminal reason in namespaced metadata; V8 must verify both keys before the
  1.0.0 upgrade. Date/author: 2026-10-17, planning agent; clarified 2026-10-23.
- D13 — Replace `TOTAL_BACKOFF_BUDGET` with a per-delay ceiling,
  `MAX_BACKOFF_DELAY` (one hour). Rationale: the total budget existed to bound
  worker-slot occupancy and to stay below the orphan-recovery threshold. Under
  D11, no worker holds a job during backoff, so neither concern applies. A
  ceiling still guards against misconfiguration (for example, a large
  multiplier) parking jobs for days. Date/author: 2026-10-17, planning agent.
- D14 — Forbid in-process retry middleware on Apalis workers at the rc
  pins. Rationale: the 2026-10-14 evidence shows it races with the storage
  fetcher. The V9 conformance check enforces this rule, and ADR-002 records it.
  Issue #550 may revisit it under 1.0.0's changed middleware order, but only
  with evidence. Date/author: 2026-10-17, planning agent.
- D15 — Use the existing Apalis JSONB `metadata` field for two namespaced
  keys: `wildside.retry.requeue_count` and `wildside.retry.dead_reason`. A
  guarded requeue increments the bounded counter, preserves unrelated metadata,
  and removes the prior dead reason. A guarded terminal-failure update writes
  `fatal` or `exhausted` to the reason key and, when needed, shrinks
  `max_attempts` in the same statement. This lets `list_dead` distinguish a
  fatal failure from policy exhaustion even when both become an unfetchable
  row. Rows without the key are `Unclassified` because status and attempts
  cannot identify their terminal cause. Rationale: Apalis replaces
  `last_result` on every acknowledgement, so requeue provenance there
  disappears after the next run; status and attempt counts alone cannot
  distinguish all terminal reasons. Keeping both values in namespaced metadata
  preserves the existing bounded plain error string in `last_result`. The
  pinned rc.8 hydrator carries metadata into `PgContext`, and its ack leaves
  the column untouched. This adds no schema object and preserves the ban on a
  separate dead-letter table (D3). The retry budget remains sourced from
  `RequeueBudgets` (D12), never metadata. Preserve all unrelated keys,
  initialize an absent counter to zero, validate it as `u32`, and treat both
  keys as rc-coupled contracts covered by V8 before the 1.0.0 upgrade (#550).
  An application-owned envelope in `last_result` was considered and rejected
  because it would change Apalis's serialized error representation. The user
  accepted the two metadata writes on 2026-10-25 Europe/Berlin. Date/author:
  2026-10-23 Europe/Berlin, implementation review; acceptance: user.
- D16 — Cap `JobFailure` messages at 1,024 UTF-8 bytes and truncate only at a
  character boundary. Rationale: the failure is payload-free and later becomes
  the bounded plain Apalis `last_result`; a byte cap gives that boundary an
  exact, reusable contract without producing invalid UTF-8. Date/author:
  2026-10-25 Europe/Berlin, implementation refinement.
- D17 — Group retry-policy constructor values in `JobRetryPolicyConfig` and
  accept that value from `JobRetryPolicy::new`. Rationale: Clippy's configured
  argument limit is four; named fields keep each policy setting clear at the
  call site while preserving constructor validation and policy behaviour. The
  invalid-configuration rstest cases also pass that value as a unit to keep the
  test helper within the same limit. Date/author: 2026-10-25 Europe/Berlin,
  implementation refinement.

## Outcomes & retrospective

EP-M0 implementation and deterministic gates completed on the 2026-10-23
Europe/Berlin candidate: the pinned dependency graph, acknowledgement source
contract, and durable scheduling probe are confirmed. The local M0 CLI review
exposed two persistence needs. `list_dead` cannot contain a job after its
successful acknowledgement, and `last_result` cannot preserve a requeue marker
across another acknowledgement. Apalis status and attempt counts also cannot
distinguish a fatal error from policy exhaustion when the stored attempt limit
exceeds the current policy. The user accepted D15 on 2026-10-25, authorizing
the two namespaced JSONB metadata keys while preserving the existing
`last_result` format. EP-M1 is active. Before marking the plan `COMPLETE`,
reconcile the architecture document's "Background Job Execution" prose with the
implemented mechanism and confirm no upstream assumption in `Conformance basis`
was falsified.

## Revision note

2026-10-25 Europe/Berlin: the user accepted D15's recommendation to persist
requeue provenance and terminal failure reasons in the two namespaced Apalis
`metadata` keys. The approval gate is resolved and EP-M1 begins. All D15
requirements and verification obligations are now unconditional; dated
pre-acceptance checkpoints below remain historical.

2026-10-23: recorded the missing-`PG_PASSWORD` diagnosis and the successful
five-prediction AXIOM-DURABLE probe after the non-forcing Cargo defaults were
added. The branch was rebased onto `origin/main`, preserving main's formatter
target and TypeDoc fixture improvements; the four requested post-rebase gates
passed. The scratch probe was removed. A final M0 sequence passed all seven
gates, and source inspection plus the resolved tree confirm AXIOM-ACK and
AXIOM-PIN. EP-M0 implementation is complete; its CLI review precedes EP-M1.

2026-08-16: revised after the expert-panel design review. Material changes:
fatal dead-lettering is now an explicit storage-level action (D4 rewritten; V5
strengthened to assert non-re-delivery); the two retry loops (tower in-process
versus storage refetch) are reconciled through the named axiom AXIOM-ATTEMPT,
an EP-M0 compile probe, and V4's total-executions assertion; the domain policy
is now the executed policy via a delegating `tower::retry::Policy` (D1
rewritten); a `TOTAL_BACKOFF_BUDGET` constructor constraint and obligation V1b
bound worker-slot occupancy and pin the orphan-recovery inequality; per-lease
attempt semantics, crash-loop risk, per-attempt logging, and the
unconfigured-orphan-recovery fact are documented; requeue and discard are
guarded compare-and-swap operations with provenance, `NotDead` errors, and
settled discard-as-delete semantics (D5); dead-letter and retry metrics are
named deliverables; `DeadJobId` newtype, `dead_at` naming, `#[non_exhaustive]`,
ordering, and payload-privacy contracts added to the port; rc-coupling canaries
(V8), attempt-mapping tests (V3b), the optional partial index (D6),
per-instance policy (D7), the operator runbook, the harness promotion path, and
the ADR split (ADR-002/ADR-003) were added. Remaining work is unchanged in
structure: EP-M0 through EP-M4 pending approval.

2026-10-13: execution began under explicit user authorization. Firecrawl
inspection of the pinned Apalis sources disproved AXIOM-ATTEMPT: the worker
tracker increments once around the complete user-middleware call, and the retry
policy only reads that count. EP-M1–M4 are paused pending review of the
proposed policy-owned attempt increment and a bounded runtime probe. D8 keeps
retry/dead-letter event counters separate from enqueue outcome and latency
metrics.

2026-10-14: planning review of the 2026-10-13 finding. That finding inverted
tower's layer order: the tracker is innermost and already counts every
in-process attempt, so the proposed policy-owned increment is rejected (D9).
The review instead found that every in-process attempt is acked separately,
leaving a re-fetchable `Failed` row during each backoff sleep. That is a
duplicate-execution hazard, and it invalidates in-process backoff at these
pins. The review also found that the build resolves rc.9/rc.8, not the
rc.7/rc.6 named here, and that the caret requirements admit the breaking
`1.0.0` final release. Two proposals await the user's ruling: D10 (exact pins;
the 1.0.0 upgrade becomes a separate item) and D11 (durable `run_at` backoff in
place of the tower retry layer). EP-M0 now carries a falsifiable,
five-prediction runtime probe (AXIOM-DURABLE). EP-M1 onward stays paused.

2026-10-17: the user accepted D10 (exact Apalis pins) and D11 (durable,
storage-scheduled backoff). The plan is rewritten around them, and a
`Read this first` section summarizes the delta for implementers. Changes:

- AXIOM-FACADE and AXIOM-ATTEMPT are retired in favour of AXIOM-PIN,
  AXIOM-DURABLE, and AXIOM-ACK, discharged by EP-M0's pin step and five-part
  runtime probe.
- D1 and D2 are superseded, D4's mechanism is settled, and D12 (requeue
  restores the budget), D13 (`MAX_BACKOFF_DELAY` replaces
  `TOTAL_BACKOFF_BUDGET`), and D14 (no in-process retry middleware) are added.
- Interfaces gain `JobFailure`, `DeadReason`, `DeadLetterError::UnknownQueue`,
  `PostgresRetryScheduler`, and the service-shaped handler wrapper. The
  `apalis` facade dependency is dropped.
- Obligations V1b, V3b, V4, V5, V6, and V8 are rewritten, and V9 (no retry
  layer) is added.
- EP-M2 is rewritten, EP-M3 gains budget restoration, and EP-M4 names the
  renamed ADR-002 (`adr-002-durable-retry-scheduling-for-apalis-jobs.md`).
- Issue [#550](https://github.com/leynos/wildside/issues/550) tracks the
  Apalis 1.0.0 upgrade.

This paragraph records the plan state before D15 was accepted. The user's
2026-10-25 acceptance resolved that approval gate; EP-M1 is now active.

2026-10-23 Europe/Berlin: reconciled the M0 local CodeRabbit findings.
Corrected the post-acknowledgement provenance assertion, clarified that the
review's October 22 UTC date and the plan's October 23 Europe/Berlin date refer
to the same period, and fixed the style finding. Apalis metadata is a
technically viable location for the requeue counter and dead reason, but its
write is outside the approved column allow-list. Proposed D15 is explicitly
pending user approval, the accepted invariant remains unchanged, and the plan
is blocked before EP-M1.

2026-10-23 Europe/Berlin: the D15 reassessment corrected the stale
current-version paragraph to state the exact rc.9/rc.8 manifest pins and
identify the caret requirements as historical. Its October 22 UTC date and the
plan's October 23 local date are consistent; the review completed after the
commit it assessed. The candidate awaits refreshed documentation gates and CLI
reassessment.

2026-10-23 Europe/Berlin: the final D15 reassessment found that status alone
cannot distinguish policy exhaustion from a fatal error when the stored Apalis
limit exceeds the current policy. Proposed D15 now records the reason in
`wildside.retry.dead_reason` alongside the requeue counter. Apalis's
`last_result` JSON representation remains unchanged; acknowledgements replace
its value with the bounded error string returned by the wrapper. The requested
PR #457 rebase onto `origin/main` was an up-to-date no-op: the branch's first
commit already directly followed the target, so no commits replayed and no
conflicts arose. The post-rebase candidate passed `make check-fmt`, `make test`
(1,851 Nextest passed, four skipped, plus the compile-fail, frontend,
workflow-contract, and Python suites), `make typecheck`, `make lint`,
`make markdownlint`, `make nixie`, and `make spelling`. The local CLI
reassessment remains pending. D15 requires explicit user approval before any
implementation milestone begins. Its October 22 review date is UTC; the
recorded commits and review use Europe/Berlin (`+02:00`) and are ordered by
their exact timestamps.
