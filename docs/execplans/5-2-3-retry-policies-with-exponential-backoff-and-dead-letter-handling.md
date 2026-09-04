# Implement retry policies with exponential backoff and dead-letter handling (backend 5.2.3)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, `Outcomes & Retrospective`, `Conformance Basis`, and
`Verification Plan` must be kept up to date as work proceeds.

Status: DRAFT

This plan covers backend roadmap item 5.2.3 only:

> Implement retry policies with exponential backoff and dead-letter handling.

The plan must be explicitly approved by the user before any implementation
milestone begins. Approval of this plan does not authorize worker consumption
or deployment (roadmap 5.3.1), trace-identifier propagation (5.2.4), or
request-path dispatch changes (the `TODO(#276)` markers in
`backend/src/domain/route_submission/mod.rs` remain in place).

## Purpose / big picture

Wildside enqueues background jobs (`GenerateRouteJob`, `EnrichmentJob`)
through the `RouteQueue` port into an Apalis-backed PostgreSQL queue
(`apalis.jobs` table). Today a job that fails during processing would be
retried using Apalis defaults (up to 25 attempts, no deliberate backoff), and
a permanently failing job would linger with no dead-letter story: nothing
bounds retries deliberately, nothing classifies errors as retryable versus
fatal, and no interface exists to inspect or requeue dead jobs.

This plan adds the retry and dead-letter policy layer:

- A pure domain retry policy that computes bounded exponential backoff delays
  with jitter and decides, per attempt and error class, whether to retry or
  dead-letter a job. The domain policy is the policy production executes: the
  adapter implements a `tower::retry::Policy` that delegates to it, so the
  verified function and the running function are the same function.
- A domain error classification for job execution failures (retryable versus
  fatal), following the precedent of
  `EnrichmentSourceError::is_retryable()`.
- A `DeadLetterQueue` driven port and a PostgreSQL adapter that lists,
  requeues, and discards dead jobs (rows in `apalis.jobs` that are `Killed`
  or that have exhausted their attempts), with guarded state transitions and
  requeue provenance.
- Adapter wiring that translates the domain policy into the Apalis 1.0-rc
  tower retry stack, persists the bounded `max_attempts` on every enqueued
  job, enforces that fatally failed jobs are dead at the storage level (not
  merely un-retried in-process), and emits retry and dead-letter metrics
  through the existing `RouteQueueMetrics` port.

Observable outcomes:

- `cargo test -p backend` demonstrates: a job whose handler always fails
  retryably is attempted exactly `max_attempts` times in total (across all
  storage fetches, within one worker lease) and is then dead; a fatal
  failure runs the handler exactly once and leaves a row that the storage
  fetcher can never pick up again; a dead job can be listed and requeued
  through the `DeadLetterQueue` port and then observably runs to success;
  requeuing a live or absent job fails cleanly. These run against embedded
  PostgreSQL via `rstest-bdd` scenarios.
- Property tests pin the backoff invariants (delays bounded above and below,
  jitter within the configured fraction, monotone non-decreasing growth
  until the cap binds, a bounded total-backoff budget, and retry decisions
  that never exceed `max_attempts`).
- `make check-fmt`, `make typecheck`, `make lint`, and `make test` pass after
  each major milestone.
- Roadmap item 5.2.3 is marked done only after every gate passes and the
  documentation updates land.

## Context and orientation

Everything below is verifiable in the working tree; no prior plan is
required reading, though
`docs/execplans/backend-5-2-2-job-structs-for-generate-route-and-enrichment.md`
records the scope boundaries this plan inherits.

Repository state relevant to this task:

- Port: `backend/src/domain/ports/route_queue.rs` defines `RouteQueue` (one
  method, `enqueue`) and `JobDispatchError` (`Unavailable`, `Rejected`) via
  the `define_port_error!` macro in
  `backend/src/domain/ports/macros.rs`. `JobDispatchError` has no
  retryable/fatal distinction; it describes enqueue-side failures only.
- Adapter: `backend/src/outbound/queue/apalis_route_queue.rs` implements the
  enqueue side. `ApalisPostgresProvider` wraps
  `apalis_postgres::PostgresStorage<serde_json::Value>` over a dedicated
  `sqlx::PgPool`; `setup` creates the `apalis.jobs` schema at runtime (no
  Diesel migration). `QueueProvider` is the crate-internal test seam with
  `FakeQueueProvider`/`FailingQueueProvider` doubles in
  `backend/src/outbound/queue/test_helpers.rs`.
- Consumer-side scaffolding: `backend/src/outbound/queue/job_decode.rs`
  provides the pure `decode_job<J>` helper. Its module documentation and
  `docs/wildside-backend-architecture.md` (section "Background Job Execution
  (Apalis Workers)") both state that the queue-processing error path applies
  "the retry/dead-letter policy owned by 5.2.3" — that seam is where this
  plan's policy attaches. Note that `decode_job` deliberately never logs
  payload contents; the same privacy stance applies to dead-letter surfaces
  in this plan.
- Jobs: `backend/src/domain/jobs/generate_route.rs` and
  `backend/src/domain/jobs/enrichment.rs` define versioned V1 envelopes. The
  module documentation of `backend/src/domain/jobs/mod.rs` currently scopes
  the module to job payloads; it must be updated when the retry policy joins
  it.
- Error-classification precedent:
  `backend/src/domain/ports/enrichment_source.rs` exposes
  `EnrichmentSourceError::is_retryable()`, and
  `backend/src/domain/overpass_enrichment_worker/attempt_error.rs` shows the
  house pattern for attempt-local error taxonomies.
- Metrics: the `RouteQueueMetrics` port with `RouteQueueOutcome`
  (`success`/`failure`) and the Prometheus adapter exist. This plan extends
  the outcome vocabulary with retry and dead-letter events (a named
  deliverable, not an opportunistic one); queue-depth and age gauges remain
  deferred to roadmap 6.x.
- No worker, `Monitor`, or `WorkerBuilder` exists anywhere in the crate, and
  nothing configures Apalis orphan recovery (`reenqueue_orphaned_after`)
  yet. Production worker consumption and orphan-recovery configuration are
  roadmap 5.3.1. Until 5.3.1 lands, a worker process that dies mid-job
  leaves the row `Running` indefinitely; this plan documents that fact
  rather than fixing it. Test-only consumption harnesses are in scope here
  because behavioural evidence of retry and dead-letter behaviour requires
  driving job execution in tests.

Facts about Apalis at the pinned versions (`apalis-core = "1.0.0-rc.7"`,
`apalis-postgres = "1.0.0-rc.6"`, `backend/Cargo.toml:34-35`), established
from docs.rs for those exact versions, the published crate sources, and the
apalis-dev repositories; these are external axioms for this plan:

- Retry middleware lives in the `apalis` facade crate
  (`apalis::layers::retry`, crate feature `retry`), not in `apalis-core`. It
  re-exports `tower::retry`: `RetryPolicy::retries(n)`,
  `.with_backoff(...)` for backoff, and `.retry_if(pred)` for conditional
  retry, and `Policy` is implementable for custom behaviour. `apalis-core`'s
  `BackoffStrategy`/`BackoffConfig` govern backend polling cadence, not task
  retries. Adopting the upstream layer requires adding the `apalis` facade
  crate as a dependency with the `retry` feature.
- The `apalis.jobs` row carries `status` (`Pending`, `Queued`, `Running`,
  `Done`, `Failed`, `Killed`), `attempts` (default 0), `max_attempts`
  (default 25), and `run_at`. The fetch function `apalis.get_jobs` selects
  `status = 'Pending' OR (status = 'Failed' AND attempts < max_attempts)`
  with `run_at < now()`. There are therefore two retry loops in play: the
  tower layer's in-process retries within one execution, and the storage
  fetcher's re-delivery of `Failed` rows. Reconciling them is a named
  verification obligation of this plan (AXIOM-ATTEMPT below).
- On acknowledgement, `calculate_status` maps `Ok` to `Done`, and an error
  with `attempt >= max_attempts` to `Killed`, otherwise `Failed`. The
  `Error::Abort => Killed` arm is present but commented out in
  apalis-postgres 1.0.0-rc.6, so abort-style fatal errors are not persisted
  as `Killed` by the library. A fatal error acked at attempt 1 of 3 becomes
  a `Failed` row with spare attempts: the storage fetcher will re-deliver
  it, and a dead-letter query keyed on exhaustion will not see it. Fatal
  dead-lettering therefore requires an explicit storage-level action by our
  wiring (Decision D4).
- There is no built-in dead-letter table or requeue API. Dead jobs are
  `Killed` rows (or exhausted `Failed` rows) that remain in `apalis.jobs`;
  listing and requeuing require direct SQL. No pruning or retention
  mechanism exists upstream at these pins.
- Backoff sleeps happen in-process inside the tower retry layer while the
  job row stays `Running` and locked; `run_at` is not rescheduled on
  failure. Attempts are persisted once, at final acknowledgement.
  Consequences: `max_attempts` is a per-lease bound — a worker crash
  discards in-process attempt history, and the recovered job restarts from
  its last persisted count, so lifetime attempts across crashes are
  unbounded and handlers must be idempotent; and long backoff chains occupy
  a worker slot for their whole duration, so the total in-process wait must
  be bounded and must remain below whatever orphan-recovery threshold 5.3.1
  configures, or the reaper would re-deliver a live, mid-backoff job and
  cause duplicate execution.

Named external axioms to confirm by compile probe before implementation
proceeds (the upstream-surprises tolerance attaches to each):

- AXIOM-FACADE: the `apalis` facade crate at 1.0.0-rc.7 composes with
  `apalis-postgres` 1.0.0-rc.6 (release candidates carry no cross-version
  guarantee; the three-way skew is exactly where a trait-bound mismatch
  would surface).
- AXIOM-ATTEMPT: the facade retry layer increments the task's shared
  `apalis_core::task::Attempt` on each in-process retry, so the final ack
  observes the cumulative count and writes `Killed` when
  `attempt >= max_attempts`, and the storage refetch loop never multiplies
  the budget for policy-managed jobs. If this axiom fails, stop and
  escalate: the wiring design changes materially.
- AXIOM-ACK: the ack path persists `attempts` and computes status as described
  above (verified in rc.6 sources; re-verified cheaply by the rc-canary
  tests, obligation V8).

Terms used in this plan:

- "Dead-letter" a job: stop retrying it and park it in a state the storage
  fetcher can never re-deliver (`Killed`, or `Failed` with exhausted
  attempts) for operator inspection and optional requeue.
- "Fatal error": a job-execution failure that retrying cannot fix (for
  example, a payload that fails validation), classified so the job
  dead-letters immediately rather than burning attempts.
- "Retryable error": a transient failure (timeout, transport, rate limit)
  worth retrying with backoff.
- "Worker lease": the period from a worker fetching a job (row locked,
  `Running`) to its final acknowledgement. Attempt counts are persisted per
  lease.

## Constraints

Hard invariants. Violation requires escalation, not workarounds.

- Do not begin implementation before the user approves this plan.
- Keep scope to roadmap item 5.2.3. Do not implement 5.2.4 (trace
  propagation), 5.3.1 (worker pools, Kubernetes deployment, queue
  partitioning, orphan-recovery configuration), or request-path dispatch.
  Production binaries must not gain a worker loop from this plan;
  consumption harnesses are test-only.
- Preserve hexagonal architecture. The retry policy and error classification
  live in the domain layer and must not import Apalis, tower, SQLx, Diesel,
  or Actix types. The `DeadLetterQueue` port is defined by the domain; the
  PostgreSQL adapter implements it in `backend/src/outbound/`. Background
  workers and caches interact with the domain exclusively via ports
  (roadmap section 5 preamble).
- The domain policy is authoritative: production retry behaviour must route
  through `JobRetryPolicy::decide`/`backoff_delay`. Substituting tower's
  own backoff arithmetic for the domain's requires an explicit
  translation-verification obligation and a Decision-log entry.
- The retry policy constructor must enforce a total-backoff budget: the
  worst-case sum of all in-process delays,
  `sum(backoff_delay(a) for a in 1..max_attempts)` at maximal jitter, must
  not exceed the named constant `TOTAL_BACKOFF_BUDGET` (initial value 60
  seconds; recorded in ADR-002 together with the requirement that this
  budget plus worst-case handler duration stays strictly below the
  orphan-recovery threshold 5.3.1 will configure).
- Do not change the `RouteQueue` trait signature. `JobDispatchError`
  variants may only be extended, not changed, and only if a milestone
  demonstrates the need; the default position is to leave enqueue-side
  errors untouched because retry classification concerns job execution, not
  enqueue.
- Do not upgrade or replace `apalis-core`, `apalis-postgres`, or `sqlx`
  pins. Adding the `apalis` facade crate at the matching 1.0.0-rc.7 version
  with the `retry` feature is the single permitted dependency addition on
  the production side; `googletest` is the single permitted dev-dependency
  addition. Any further dependency need triggers escalation.
- Do not alter or migrate the Apalis-owned schema shape (`apalis.jobs`
  columns, statuses, functions) and do not add Diesel migrations for Apalis
  tables. One additive exception is pre-authorized: the dead-letter adapter
  may create a partial index supporting the dead-jobs predicate, executed
  through the adapter's setup path alongside `PostgresStorage::setup`, if
  and only if milestone EP-M3's `EXPLAIN` check shows a sequential scan
  (Decision D6). Introducing a separate dead-letter table is out of scope
  (Decision D3).
- Dead-letter state transitions must be guarded compare-and-swap updates:
  `requeue` and `discard` take effect only when the row currently satisfies
  the dead predicate, and an ineffective update surfaces as a port error,
  never as silence.
- Automated requeue is forbidden. `requeue` is an operator action; any
  future automation must carry a per-job requeue budget (recorded in
  ADR-003). Requeue provenance must be recorded so repeat offenders are
  visible.
- Dead-letter surfaces (the `DeadJob` type, logs, metrics) must not expose
  raw job payloads; persisted error strings must be size-bounded. This
  matches the existing `decode_job` privacy stance.
- Behavioural tests must not assert timing magnitudes (only counts,
  orderings, and terminal states), and the test consumption harness must
  have a hard iteration and deadline cap so a wiring bug fails fast instead
  of hanging the suite.
- Versioned job envelopes and their JSON shape (pinned by `insta`
  snapshots from 5.2.2) must not change.
- Keep documentation in en-GB-oxendict style per
  `docs/documentation-style-guide.md`; wrap prose at 80 columns and code at
  120 columns; sentence-case headings.
- Keep source files below 400 lines; split modules before finishing a
  milestone if needed.
- Run gates sequentially (build caching), never in parallel, and capture
  long output with `tee` to
  `/tmp/$ACTION-wildside-$(git branch --show-current).out`. Prefer Makefile
  targets. Full gate runs are delegated to the `scrutineer` subagent.
- Commit each milestone only after its gates pass. Do not amend earlier
  commits; add new ones.
- Do not mark roadmap item 5.2.3 done until all gates and reviews are clean.

## Tolerances (exception triggers)

- Scope budget: stop and escalate if the implementation needs more than
  twelve production source files changed or more than 900 net non-test
  lines. Tests, snapshots, feature files, and documentation are excluded
  from the budget.
- Port shape: stop and escalate before changing `RouteQueue`, existing
  `JobDispatchError` variants, or any 5.2.2 job struct field.
- Dependencies: the `apalis` facade crate (with `retry` feature, version
  matching `apalis-core` 1.0.0-rc.7) and `googletest` (dev-dependency) are
  pre-authorized by this plan once approved. Stop and escalate for anything
  else.
- Upstream surprises: if any named axiom (AXIOM-FACADE, AXIOM-ATTEMPT, AXIOM-ACK)
  fails its compile probe or canary, or the Apalis rc APIs otherwise differ
  materially from "Context and orientation" (for example, `max_attempts`
  cannot be set per enqueued task), stop, record the discovery, and
  escalate with options rather than upgrading pins.
- Iterations: if a red test still fails after three green attempts, or a
  gate fails three consecutive runs on the same cause, stop and escalate.
- Ambiguity: if retry semantics interact with idempotency in a way not
  settled here, stop and present options.

## Risks

- Risk: Apalis 1.0-rc APIs are release candidates and may not match docs.rs
  exactly at compile time, and the facade (rc.7) over `apalis-postgres`
  (rc.6) is an unverified three-way skew.
  Severity: medium. Likelihood: medium.
  Mitigation: milestone EP-M0 includes the compile probe as an explicit
  step (not merely a mitigation note); the upstream-surprises tolerance
  names the axioms it protects.
- Risk: in-process backoff holds a job locked and occupies a worker slot
  for the whole retry chain; interacting badly with a future
  orphan-recovery threshold would cause duplicate execution of live jobs.
  Severity: high. Likelihood: medium.
  Mitigation: the `TOTAL_BACKOFF_BUDGET` constructor constraint (obligation
  V1b) makes the bound structural; ADR-002 records the inequality 5.3.1
  must respect; the retry counter metric gives the occupancy signal.
- Risk: a worker crash mid-lease discards in-process attempt history, so a
  handler that crashes the process (out-of-memory, abort) can loop forever
  without ever dead-lettering.
  Severity: medium. Likelihood: low-to-medium.
  Mitigation: per-attempt structured logging (job id, attempt number) is
  the artefact that survives a crash; the operator runbook (EP-M4) includes
  the crash-loop detection query; the per-lease semantics are documented
  rather than implied. A structural fix (persisting attempts per attempt)
  belongs upstream or to 5.3.1.
- Risk: behavioural tests need test-only job consumption, which could drift
  into building 5.3.1's worker early, or diverge from what 5.3.1 builds.
  Severity: low. Likelihood: medium.
  Mitigation: the harness lives under `backend/tests/support/`, is not part
  of the production binary, and its worker-construction function is named
  as the reference wiring 5.3.1 must promote or consciously diverge from.
- Risk: `rstest-bdd` scenarios with embedded PostgreSQL and real sleeps
  could be slow or flaky.
  Severity: medium. Likelihood: medium.
  Mitigation: test policies use millisecond-scale delays; scenarios assert
  counts, orderings, and terminal states only; the harness has hard
  iteration and deadline caps; embedded-PostgreSQL setup reuses
  `backend/tests/support/embedded_postgres.rs`.
- Risk: `apalis.jobs` grows without bound (`Done` and `Killed` rows are
  never pruned upstream), degrading the dead-letter listing over time.
  Severity: low (now), medium (later). Likelihood: high eventually.
  Mitigation: `list_dead` is ordered and limited; the `EXPLAIN` check and
  optional partial index (D6) keep the query plan honest; row retention is
  explicitly recorded as unowned here and proposed for roadmap section 6
  (observability/operations) rather than silently ignored.

## Skills and reference documents

Skills to load when implementing: `leta`, `rust-router` (then
`rust-errors` for the error taxonomy and `rust-async-and-concurrency` for
the tower layer work), `hexagonal-architecture`, `proptest`,
`rust-unit-testing`, `rust-unused-code` (for the pre-5.3.1 public factory
seam), `arch-decision-records` (for the ADRs), `commit-message`,
`pr-creation`, `en-gb-oxendict`.

Repository documents to read first:

- `docs/wildside-backend-architecture.md` — "Background Job Execution
  (Apalis Workers)" section (retry/dead-letter design intent, scope split
  between 5.2.1–5.2.4 and 5.3.1, the `decode_job` seam, and the promise of
  per-job-type queues that shapes the policy-per-adapter decision D7).
- `docs/backend-roadmap.md` — section 5.2 and 5.3 (scope overlap with
  5.3.1's "bounded retries, and dead-letter handling").
- `docs/developers-guide.md` — "Apalis queue adapter boundaries",
  "Background job payloads", "Queue build requirements", "Queue test
  infrastructure".
- `docs/rstest-bdd-users-guide.md` and
  `docs/pg-embed-setup-unpriv-users-guide.md` — behavioural-test and
  embedded-PostgreSQL conventions.
- `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/rust-doctest-dry-guide.md`,
  `docs/complexity-antipatterns-and-refactoring-strategies.md` — testing
  and structure conventions.
- `docs/documentation-style-guide.md` — ADR template ("an ADR captures one
  accepted decision") and style rules.

## Conformance basis

Upstream artefacts (no Terms of Reference document exists for this
repository; the roadmap and architecture document are the governing
artefacts):

- `docs/backend-roadmap.md` item 5.2.3 (identifier ROAD-5.2.3) and the
  section 5 preamble constraint that workers interact with the domain only
  via ports (ROAD-5-PORTS).
- `docs/wildside-backend-architecture.md`, "Background Job Execution
  (Apalis Workers)" (identifier ARCH-JOBS): bounded retries with
  exponential backoff, dead-letter after max attempts, per-job-type queue
  configuration, and the 5.2.3/5.3.1 scope split.
- `docs/execplans/backend-5-2-2-job-structs-for-generate-route-and-enrichment.md`
  scope reservations (EP-522-SCOPE): job envelope stability, deferred
  retry/dead-letter ownership.
- New ADRs produced by this plan:
  `docs/adr-002-apalis-in-process-retry-with-bounded-backoff.md` (ADR-002,
  the retry mechanism: tower layer, in-process backoff, total-backoff
  budget, fatal short-circuit) and
  `docs/adr-003-dead-letters-as-apalis-rows-behind-a-domain-port.md`
  (ADR-003, the dead-letter representation, guarded transitions, requeue
  provenance and the no-automated-requeue rule). Split per the style
  guide's one-decision-per-ADR rule.

Trace links:

```plaintext
ROAD-5.2.3 -> ARCH-JOBS -> EP-M1 -> backend/src/domain/jobs/retry.rs (policy + proptest invariants V1-V3)
ROAD-5.2.3 -> ARCH-JOBS -> EP-M2 -> retry wiring + max_attempts + fatal kill (BDD: queue_retry_policy.feature, V4-V5)
ROAD-5.2.3 -> ARCH-JOBS -> EP-M3 -> DeadLetterQueue port + PostgreSQL adapter (BDD: queue_dead_letter.feature, V6)
ROAD-5-PORTS -> EP-M1..M3 -> domain imports audit (no adapter types in domain)
EP-522-SCOPE -> EP-M2 -> insta snapshots unchanged for V1 envelopes (V7)
AXIOM-FACADE/AXIOM-ATTEMPT/AXIOM-ACK -> EP-M0 compile probe + V8 rc-canary tests
ADR-002/ADR-003 -> EP-M4 -> docs updates + contents.md registration
```

## Interfaces and dependencies

Domain layer (new module `backend/src/domain/jobs/retry.rs`, re-exported
from `backend/src/domain/jobs/mod.rs`, whose module documentation is
updated to cover policy as well as payloads; names indicative, refine
during implementation without changing intent):

```rust
/// Why a job execution attempt failed, as classified by the handler.
pub enum JobErrorKind {
    /// Transient failure; retrying with backoff may succeed.
    Retryable,
    /// Permanent failure; retrying cannot succeed.
    Fatal,
}

/// A jitter sample in the unit interval, validated at construction.
pub struct JitterSample(f64); // JitterSample::new(x) errors unless 0.0 <= x <= 1.0

/// Bounded exponential backoff policy for background jobs.
pub struct JobRetryPolicy {
    max_attempts: NonZeroU32,
    base_delay: Duration,
    multiplier: f64,       // >= 1.0, validated
    max_delay: Duration,
    jitter_fraction: f64,  // 0.0..=1.0, validated; delay varies +/- this fraction
}

pub enum RetryDecision {
    /// Retry after the given delay.
    Retry { delay: Duration },
    /// Stop retrying; the job is dead-lettered.
    DeadLetter,
}

impl JobRetryPolicy {
    /// Errors unless attempts are non-zero, multiplier >= 1.0, jitter in
    /// [0, 1], and the worst-case total backoff fits TOTAL_BACKOFF_BUDGET.
    pub fn new(...) -> Result<Self, JobRetryPolicyBuildError>;
    pub fn decide(&self, attempt: NonZeroU32, kind: JobErrorKind, jitter: JitterSample) -> RetryDecision;
    pub fn backoff_delay(&self, attempt: NonZeroU32, jitter: JitterSample) -> Duration;
    pub fn max_attempts(&self) -> NonZeroU32;
}
```

The jitter sample maps linearly onto `[-jitter_fraction, +jitter_fraction]`
around the nominal delay (0.5 is the midpoint, i.e. no jitter); the policy
stays pure and deterministic under test and the adapter supplies
randomness. `JobRetryPolicyBuildError` follows the style of
`EnrichmentJobBuildError`. Handlers map source errors through a stated
convention — `impl From<&EnrichmentSourceError> for JobErrorKind` honouring
`is_retryable()` — so 5.3.1's handlers inherit the bridge rather than
inventing it.

Domain port (new file `backend/src/domain/ports/dead_letter_queue.rs`,
error via `define_port_error!`):

```rust
/// Opaque identifier for a dead job; the storage representation is an
/// adapter detail.
pub struct DeadJobId(String);

/// A job parked after exhausting retries or failing fatally.
#[non_exhaustive]
pub struct DeadJob {
    pub id: DeadJobId,
    pub job_type: String,
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
    async fn requeue(&self, id: &DeadJobId) -> Result<(), DeadLetterError>;
    /// Guarded like `requeue`; deletes the row (Decision D5).
    async fn discard(&self, id: &DeadJobId) -> Result<(), DeadLetterError>;
}
```

`DeadLetterError` variants: `NotDead { message }` (guard failed: absent or
not currently dead) and `Unavailable { message }` (storage failure). The
expected eventual driver of this port is an operator/admin surface in
roadmap section 6 (and possibly 5.3.1 worker-side tooling); until then its
only callers are behavioural tests, which is an accepted, documented gap —
the 03:00 operator interface is SQL, and EP-M4's runbook provides it.

Outbound adapter (new file
`backend/src/outbound/queue/postgres_dead_letter.rs`):
`PostgresDeadLetterQueue` holding an `sqlx::PgPool`. The dead predicate is
`status = 'Killed' OR (status = 'Failed' AND attempts >= max_attempts)`.
`list_dead` orders by `done_at DESC` with `LIMIT`. `requeue` executes a
guarded update — `SET status = 'Pending', attempts = 0, run_at = now()`,
`WHERE id = $1 AND (<dead predicate>)` — and records provenance by
prepending a bounded `requeued: N` marker to `last_result`; zero rows
affected maps to `NotDead`. `discard` is a guarded `DELETE ... RETURNING`,
logging the returned row (without payload) for forensics.

Outbound wiring (extend `backend/src/outbound/queue/`, for example
`retry_layer.rs`):

- A `tower::retry::Policy` implementation that delegates every decision to
  `JobRetryPolicy::decide`/`backoff_delay` (the adapter draws the jitter
  sample), so the domain policy is the executed policy. Tower's
  `ExponentialBackoffMaker` is a fallback only; using it requires a
  translation-verification obligation and a Decision-log entry.
- Fatal enforcement: on `JobErrorKind::Fatal` the wiring both declines the
  in-process retry and makes the row dead at the storage level (a guarded
  `UPDATE apalis.jobs SET status = 'Killed'` through the adapter's pool, or
  driving the shared attempt counter to `max_attempts` before ack if
  AXIOM-ATTEMPT's probe shows that is reliable — settled in EP-M2 and recorded
  in the Decision log). Returning `AbortError` is an optional strengthening
  if usable at this rc; the classification predicate alone must be
  sufficient, so obligation V5 does not depend on `AbortError`.
- Enqueue-side persistence of the policy's `max_attempts` on the task
  context. One `JobRetryPolicy` is carried per queue-adapter instance (that
  is, per job family/queue), matching the architecture document's
  per-job-type queue configuration; there is no global constant (Decision
  D7).
- Per-attempt structured logging (job id, attempt number, error kind) — the
  only attempt artefact that survives a worker crash — and retry and
  dead-letter counters through the extended `RouteQueueMetrics` outcome
  vocabulary.

The retry-layer factory will be a public item consumed, until 5.3.1, only
by the integration-test harness; that public export is the deliberate
5.3.1 seam (handle per the `rust-unused-code` skill), and the harness's
worker-construction function is the reference wiring 5.3.1 must promote
into `backend/src/outbound/queue/` or record why it diverged.

Production dependency addition:

```toml
apalis = { version = "1.0.0-rc.7", default-features = false, features = ["retry"] }
```

Dev-dependency addition: `googletest = "0.13"` (or current compatible
release) for matcher-based assertions alongside `pretty_assertions`.

## Verification plan

External axioms: AXIOM-FACADE, AXIOM-ATTEMPT, and AXIOM-ACK as defined in "Context
and orientation", plus: tower's retry machinery drives `Policy`
implementations per its documentation, and embedded PostgreSQL behaves
like production PostgreSQL for the SQL used. Third-party internals are not
verified; repository-owned logic that builds on these interfaces is
verified against the real interface (embedded PostgreSQL, real Apalis
storage) at the behavioural tier, and the rc-coupling canaries (V8) turn
silent upstream drift into test failures.

Obligations:

- Obligation V1 (backoff bounded above and below): for every attempt `a`
  in `1..=max_attempts` and jitter sample `j`,
  `nominal(a) * (1 - jitter_fraction) <= backoff_delay(a, j) <=
  min(nominal(a), max_delay) * (1 + jitter_fraction)` where `nominal(a)`
  is the capped exponential curve. The lower bound is material: `>= 0`
  alone would be vacuous for an unsigned `Duration`.
  Method: property test (`proptest`) over generated policies, attempts,
  and jitter samples.
  Rationale: the invariant spans a continuous input domain; examples
  cannot cover it.
  Artefact: `backend/src/domain/jobs/retry/tests/properties.rs` (or inline
  `tests` module, matching house layout).
  Evidence: `cargo test -p backend retry` — fails before implementation
  (red), passes after.
  Non-vacuity: generators emit policies across base delays from 1 ms to
  minutes, multipliers 1.0–10.0, and jitter 0.0–1.0; a seeded fault
  (temporarily removing the cap clamp) must make the property fail.
- Obligation V1b (total budget bounded): construction fails whenever the
  worst-case total in-process backoff exceeds `TOTAL_BACKOFF_BUDGET`, and
  for every successfully constructed policy the property
  `sum(backoff_delay(a, max_jitter)) <= TOTAL_BACKOFF_BUDGET` holds.
  Method: property test over generated configurations, plus parameterized
  boundary cases just under and just over the budget.
  Non-vacuity: the generator must produce both accepted and rejected
  configurations (classification recorded); weakening the constructor
  check must fail the over-budget cases.
- Obligation V2 (monotone growth pre-cap): with jitter fixed at the
  midpoint, `backoff_delay(a+1) >= backoff_delay(a)` until the cap is
  reached.
  Method: property test plus parameterized `rstest` cases at the boundary
  (attempt where the cap first binds).
  Non-vacuity: generator classification records the fraction of cases
  where the cap binds; both classes (capped, uncapped) must be exercised;
  negative control: inverting the multiplier must fail the property.
- Obligation V3 (decision totality and bounds): `decide` returns
  `DeadLetter` whenever `kind == Fatal` or `attempt >= max_attempts`, and
  `Retry` otherwise; no path yields a retry beyond `max_attempts`.
  Method: exhaustive parameterized tests over the finite classification
  (`Fatal`/`Retryable` × attempt below/at/above the bound) plus a property
  test over attempt ranges. `googletest` matchers for the decision shape,
  `pretty_assertions` for equality.
  Rationale: the decision function is a small finite state map; exhaustive
  partitioning plus properties is proportionate. No bounded model checking
  (`kani`) or proof (`verus`) is warranted: the logic has no unsafe code,
  no unbounded state, and no lemma beyond the enumerated partitions — this
  is recorded as the explicit justification required when declining
  heavier rigour.
  Non-vacuity: each partition has a witness case; a seeded off-by-one
  fault in the bound comparison must be rejected.
- Obligation V3b (attempt-counter alignment): the adapter maps the
  domain's 1-based attempt numbering onto Apalis's attempt counter such
  that the final ack sees `attempt >= max_attempts` exactly when the
  domain policy has decided `DeadLetter` on exhaustion.
  Method: parameterized adapter unit test over the mapping at the
  boundaries (first attempt, last attempt, one past).
  Rationale: an off-by-one here silently grants or steals an attempt and
  no other obligation would localize it.
  Non-vacuity: shifting the mapping by one in either direction must fail
  the test.
- Obligation V4 (persisted retry bound, single lease): a job enqueued
  through the adapter carries the policy's `max_attempts` in
  `apalis.jobs.max_attempts`, and a handler that always fails retryably is
  executed exactly `max_attempts` times in total across all storage
  fetches within one worker lease, after which the row is dead (per the
  dead predicate) and is never re-delivered by further polling. The
  per-lease scoping is deliberate: a worker crash resets in-process
  history (documented per-lease semantics), and this obligation does not
  claim otherwise.
  Method: behavioural test (`rstest-bdd`) against embedded PostgreSQL with
  the capped test harness; assertions on the handler's observed run count,
  the row's `attempts` and `status` columns, and on continued polling
  delivering nothing.
  Non-vacuity: the scenario asserts the handler ran more than once (a
  policy that never retries fails) and that extra polling after death
  yields no further runs (a multiplied two-loop budget fails); a control
  scenario with a succeeding handler must end `Done` with one attempt.
- Obligation V5 (fatal short-circuit is storage-dead): a handler returning
  a fatal error runs exactly once, and the resulting row satisfies the
  dead predicate and is never re-delivered by continued polling — not
  merely "the harness stopped polling".
  Method: behavioural test as V4 with a fatal-classified failure,
  including a post-failure polling phase.
  Non-vacuity: misclassifying fatal as retryable fails the run-count
  assertion; omitting the storage-level kill fails the re-delivery
  assertion (the storage fetcher would re-deliver a `Failed` row with
  spare attempts).
- Obligation V6 (dead-letter round trip and guards): a dead job appears in
  `DeadLetterQueue::list_dead` with its attempt count, bounded last error,
  and requeue count; `requeue` makes it run again and succeed (proven by
  observing execution, not a status flip) and increments its provenance
  count; `discard` removes it; requeuing a live or absent job returns
  `NotDead` and changes nothing; listing ignores live jobs.
  Method: behavioural test (`rstest-bdd`) with embedded PostgreSQL.
  Non-vacuity: listing before any failure must be empty (witness that the
  query filters live jobs); the guard scenarios are negative controls by
  construction.
- Obligation V7 (snapshot stability): V1 job envelope snapshots from 5.2.2
  are byte-identical after this plan; any new operator-facing rendering of
  dead jobs is snapshot-pinned with `insta`.
  Method: existing `insta` suites re-run; new snapshots only for new
  multivariant output.
  Non-vacuity: `cargo insta test` fails on any drift by construction.
- Obligation V8 (rc-coupling canaries): the `apalis.jobs` columns, status
  literals, fetch-eligibility predicate behaviour, and ack semantics this
  plan's SQL depends on exist and behave as assumed at the pinned
  versions.
  Method: cheap embedded-PostgreSQL integration tests that set up Apalis
  storage and assert the schema facts directly (column presence, a
  `Failed`-with-spare-attempts row is fetchable, a `Killed` row is not).
  Rationale: a future pin bump must fail tests rather than silently
  corrupt retry or dead-letter behaviour; ADR-002/ADR-003 list these
  assumptions with an "on 1.0-final upgrade, re-verify" note.
  Non-vacuity: each canary asserts a positive and a negative case (for
  example, fetchable versus not-fetchable), so a vacuously green canary
  requires both sides to break in tandem.

If implementation reveals an invariant not listed here (for example, an
ordering constraint between requeue and concurrent fetch beyond the
guarded-update design), return to this section and extend it before
continuing.

## Plan of work

### Milestone EP-M0 — approval, baseline audit, and compile probe

Confirm plan approval. Run `scrutineer` for a baseline gate run on the
untouched branch and record results in `Progress` (known issue: local
Whitaker lint suite may be ahead of CI's pin; record, do not chase).

Then run the compile probe as an explicit step, not a side note: add the
`apalis` facade dependency in a scratch commit (or a temporary test), and
confirm AXIOM-FACADE (facade rc.7 composes with `apalis-postgres` rc.6: a
worker builder with `.retry(...)` over `PostgresStorage` type-checks) and
AXIOM-ATTEMPT (inspect the facade retry layer's source or a minimal runtime
probe to confirm the shared `Attempt` is incremented per in-process retry).
Record findings in `Surprises & discoveries`. If either axiom fails, stop
and escalate before any milestone work.

Plateau: clean tree, recorded baseline, axioms confirmed or escalated.
Recovery: drop the scratch commit.

### Milestone EP-M1 — domain retry policy and classification (red-green)

Red: add failing `rstest` unit tests, `googletest`/`pretty_assertions`
assertions, and `proptest` properties for `JobRetryPolicy`,
`RetryDecision`, `JobErrorKind`, and `JitterSample` (obligations V1, V1b,
V2, V3). Add `googletest` to `[dev-dependencies]`.

Green: implement `backend/src/domain/jobs/retry.rs` (pure; no adapter
imports). Constructor validation per the interface section, including the
`TOTAL_BACKOFF_BUDGET` check. Add the
`From<&EnrichmentSourceError> for JobErrorKind` bridge. Update the
`backend/src/domain/jobs/mod.rs` module documentation to cover policy as
well as payloads.

Refactor: documentation comments with examples per house style; keep files
under 400 lines.

Plateau: domain policy fully tested and inert (nothing consumes it yet —
coherent because it is a pure library addition). Gates via `scrutineer`;
commit.

### Milestone EP-M2 — Apalis retry wiring, bounded attempts, fatal kill

Red: add the BDD feature `backend/tests/features/queue_retry_policy.feature`
(embedded below) and step bindings in
`backend/tests/queue_retry_policy_bdd.rs`; add the test-only consumption
harness under `backend/tests/support/` (worker built from the facade retry
layer + storage, driven with hard iteration and deadline caps). Add the V8
rc-canary tests and the V3b attempt-mapping unit tests. Scenarios fail
because no wiring exists.

Green: add the `apalis` facade dependency (feature `retry`); implement the
custom `tower::retry::Policy` delegating to the domain policy; enqueue-side
`max_attempts` persistence; fatal enforcement (settle the mechanism —
guarded storage-level kill versus attempt-counter drive — per the
AXIOM-ATTEMPT probe results, and record it in the Decision log; the
`retry_if`-style classification must suffice without `AbortError`);
per-attempt structured logging; retry and dead-letter counters via the
extended `RouteQueueOutcome`.

Refactor: shape the harness's worker-construction function as the named
5.3.1 promotion candidate.

Feature specification (kept in sync with the implementation):

```gherkin
Feature: Queue retry policy with exponential backoff

  Scenario: A transiently failing job is retried with backoff then succeeds
    Given a queue with a retry policy of 4 attempts and millisecond backoff
    And a job whose handler fails twice then succeeds
    When the worker processes the queue
    Then the handler runs three times
    And the job completes with status "Done"

  Scenario: A persistently failing job exhausts its attempts and is dead-lettered
    Given a queue with a retry policy of 3 attempts and millisecond backoff
    And a job whose handler always fails with a retryable error
    When the worker processes the queue
    Then the handler runs exactly three times in total
    And the job is recorded as dead
    And further polling delivers no more runs

  Scenario: A fatally failing job is dead-lettered immediately
    Given a queue with a retry policy of 3 attempts and millisecond backoff
    And a job whose handler fails with a fatal error
    When the worker processes the queue
    Then the handler runs once
    And the job is recorded as dead
    And further polling delivers no more runs
```

Plateau: retry policy enforced end to end under test; production behaviour
unchanged except enqueued rows now carry the bounded `max_attempts`
(persisted-format note: this only tightens the existing column's
per-row value; deployed rows with the default 25 remain valid). Gates;
commit.

### Milestone EP-M3 — dead-letter port and PostgreSQL adapter

Red: add `backend/tests/features/queue_dead_letter.feature` (embedded
below) and step bindings in `backend/tests/queue_dead_letter_bdd.rs`; add
failing unit tests for `DeadJobId`, the port error type, and the guard
semantics.

Green: define the `DeadLetterQueue` port; implement
`PostgresDeadLetterQueue` with the guarded compare-and-swap updates,
requeue provenance, discard-as-delete (Decision D5), ordered and limited
listing, and size-bounded, payload-free `last_error`. Run the `EXPLAIN`
check against a seeded table (a few thousand rows) and create the partial
index through the adapter setup path only if a sequential scan appears
(Decision D6); record the outcome either way.

Feature specification:

```gherkin
Feature: Dead-letter listing, requeue, and discard

  Scenario: Dead jobs are listed with their failure context
    Given a job has been dead-lettered after exhausting its retries
    When the operator lists dead jobs
    Then the listing contains the job with its attempt count and last error

  Scenario: A requeued dead job runs again and can succeed
    Given a job has been dead-lettered after exhausting its retries
    And the underlying fault has been fixed
    When the operator requeues the dead job
    And the worker processes the queue
    Then the job completes with status "Done"
    And the listing records one prior requeue for that job

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

Plateau: dead-letter handling usable by future operators/workers through a
domain port. Gates; commit.

### Milestone EP-M4 — documentation, ADRs, runbook, roadmap closure

Write ADR-002 and ADR-003 (named under "Conformance basis") per the
style-guide template and the `arch-decision-records` skill, recording
decisions D1–D7, the rc-coupled assumptions with their "on 1.0-final
upgrade, re-verify" note, and a short pin-bump rehearsal subsection naming
the V8 canaries and the decisions to re-read when Apalis 1.0 final lands.

Update `docs/wildside-backend-architecture.md` ("Background Job
Execution") from aspiration to implemented reality, including the
per-lease attempt semantics and the backoff-budget/orphan-recovery
inequality 5.3.1 must respect. Update `docs/developers-guide.md` ("Apalis
queue adapter boundaries" and test-infrastructure sections) with the new
symbols, dependency, conventions, and an operator runbook section: the SQL
to list dead jobs, requeue one, discard one, and detect the crash-loop
signature (rows repeatedly returning to `Pending`/`Running` with stale
`attempts`), plus a note naming the roadmap item expected to deliver a
proper operator surface. Register both ADRs in `docs/contents.md`.
`docs/users-guide.md` is expected to need no change (queue internals are
not client-visible); confirm and record in the Decision log. Propose a
roadmap note for row retention/pruning ownership under section 6. Mark
roadmap item 5.2.3 done with an execution note. Full gates via
`scrutineer` including Markdown lint and Mermaid validation; commit; open
the implementation pull request.

Plateau: feature complete, documented, roadmap closed.

## Milestones and plateaus — conformance checks

At each milestone boundary check: ROAD-5-PORTS holds (no adapter imports in
`backend/src/domain/`); ARCH-JOBS scope split intact (no production worker
loop, no orphan-recovery configuration); no unapproved dependency, public
interface, or persisted-format change beyond the `max_attempts` tightening
and (if triggered) the D6 partial index named in this plan; V1 envelope
snapshots unchanged; trace links in `Conformance basis` current.

Compatibility decision: none required. All touched APIs are pre-1.0,
application-internal surfaces; no aliases, shims, or dual implementations
are permitted. The persisted-format question is confined to
`apalis.jobs.max_attempts` (an existing column whose per-row value this
plan sets explicitly; deployed rows with the default 25 remain valid) and
the optional additive partial index (D6).

## Concrete steps

Work from the repository root. Representative commands (implementers adjust
test names as they land):

```bash
git branch --show-current   # expect 5-2-3-retry-policies-with-exponential-backoff-and-dead-letter-handling
cargo test -p backend retry 2>&1 | tee /tmp/test-wildside-$(git branch --show-current).out
cargo test -p backend --test queue_retry_policy_bdd 2>&1 | tee /tmp/bdd-wildside-$(git branch --show-current).out
cargo test -p backend --test queue_dead_letter_bdd 2>&1 | tee /tmp/dlq-wildside-$(git branch --show-current).out
```

Full gates (delegated to `scrutineer`, sequential): `make check-fmt`,
`make typecheck`, `make lint`, `make test`, plus `make markdownlint` and
`make nixie` for documentation milestones.

## Validation and acceptance

Red-Green-Refactor evidence to record per milestone: the red command and
its expected failure (missing symbol or failing scenario), the green
command passing, and the refactor re-run. BDD scenarios above are the
acceptance behaviour; unit and property tests discharge V1–V3b;
behavioural tests discharge V4–V6 and V8; `insta` re-runs discharge V7.

Quality criteria: all four Makefile gates pass; obligations V1–V8
discharged with non-vacuity evidence recorded in `Artefacts and notes`;
CodeRabbit review (if requested) has no unresolved in-scope findings;
roadmap 5.2.3 ticked with an execution note.

## Idempotence and recovery

All steps are additive and re-runnable. Embedded PostgreSQL tests provision
throwaway clusters; re-running is safe. If a milestone fails mid-way,
`git status` and the committed plateau of the previous milestone define the
recovery point; revert uncommitted changes rather than patching forward.
The only destructive operation introduced is `discard` (a guarded delete of
a dead row), which exists solely behind the port and its tests.

## Progress

- [x] (2026-08-16) Reconnaissance completed: code recon, documentation
  recon, and Apalis 1.0-rc capability research recorded in this plan.
- [x] (2026-08-16) Plan drafted.
- [x] (2026-08-16) Expert-panel design review completed (structural,
  contract, scaling, operational, alternative-futures, and viability
  lenses); all findings folded into this revision.
- [ ] EP-M0 approval, baseline audit, and compile probe.
- [ ] EP-M1 domain retry policy and classification.
- [ ] EP-M2 Apalis retry wiring, bounded attempts, fatal kill.
- [ ] EP-M3 dead-letter port and PostgreSQL adapter.
- [ ] EP-M4 documentation, ADRs, runbook, roadmap closure.

## Surprises & discoveries

- Observation: apalis-postgres 1.0.0-rc.6 does not persist abort errors as
  `Killed` (the `calculate_status` arm is commented out upstream), so a
  fatal failure acked with spare attempts becomes a re-deliverable
  `Failed` row.
  Evidence: published crate source for rc.6 (`ack.rs`), and the
  `get_jobs` predicate re-fetching `Failed` rows with
  `attempts < max_attempts`.
  Impact: fatal dead-lettering requires an explicit storage-level action
  by our wiring; the dead predicate alone cannot see an early fatal
  failure (Decisions D4; obligation V5).
- Observation: the Apalis retry layer performs backoff in-process while
  the job row stays locked; `run_at` is not rescheduled on failure, and
  attempts persist only at final acknowledgement.
  Evidence: `ack.sql` in the published rc.6 crate updates only
  status/attempts/last_result/done_at; tower `Retry` awaits the backoff
  future within the same execution.
  Impact: delays must fit a total budget below any future orphan-recovery
  threshold; `max_attempts` is a per-lease bound; per-attempt logging is
  the only crash-surviving artefact (Decisions D2; obligations V1b, V4).
- Observation: nothing in this repository configures Apalis orphan
  recovery today.
  Evidence: `reenqueue_orphaned_after` appears nowhere in `backend/src`.
  Impact: until 5.3.1, a dead worker leaves its job `Running`
  indefinitely; recorded in "Context and orientation" and in ADR-002
  rather than assumed away.

## Decision log

- D1 — Adopt the upstream Apalis tower retry machinery, with a custom
  `tower::retry::Policy` that delegates to the domain `JobRetryPolicy`.
  Rationale: the facade layer is the supported mechanism at these pins and
  integrates with the worker builder 5.3.1 will use; delegating keeps the
  verified domain function authoritative instead of maintaining a parallel
  spec in tower's backoff maker. Depending on tower directly would mean
  reimplementing the apalis-aware glue (request cloning, attempt
  bookkeeping) — exactly the brittle rc-coupled code this choice avoids.
  Date/author: 2026-08-16, planning agent (subject to user approval of
  this plan).
- D2 — Accept in-process backoff under a hard total budget; defer
  persisted (`run_at`-based) backoff.
  Rationale: the honest constraint is that rc.6 exposes no failure-time
  hook with which to reschedule `run_at` (the operator-path SQL in D3
  already writes to the library-managed table, so schema purity is not the
  argument). Building the hook means owning custom ack-path middleware
  that 5.3.1 will restructure. The strongest alternative — durable
  graphile-worker-style rescheduling — buys crash-durable backoff and
  freed worker slots at that cost; at the mandated small delay caps the
  durability delta is marginal. Revisit trigger, recorded in ADR-002:
  revisit if any production policy's worst-case total in-process wait
  approaches `TOTAL_BACKOFF_BUDGET`, or if 5.3.1 load tests observe
  worker-slot exhaustion during backoff.
  Date/author: 2026-08-16, planning agent.
- D3 — Dead letters are rows in `apalis.jobs` (status `Killed` or
  exhausted `Failed`), surfaced through a domain `DeadLetterQueue` port;
  no separate table. Dead-letter surfaces never expose raw payloads
  (privacy stance inherited from `decode_job`); operators correlate via
  job id, job type, and the bounded last-error text.
  Rationale: matches Apalis's model and graphile-worker prior art; avoids
  schema divergence from library-managed tables; the port isolates the
  representation so a future table swap stays adapter-local.
  Date/author: 2026-08-16, planning agent.
- D4 — Fatal errors are made dead at the storage level by our wiring, not
  by relying on Apalis abort persistence.
  Rationale: the abort-to-`Killed` path is incomplete at rc.6, and a
  fatal `Failed` row with spare attempts would be re-delivered by the
  storage fetcher. The wiring must perform a guarded kill (or drive the
  attempt counter to exhaustion, per the AXIOM-ATTEMPT probe); either way the
  dead predicate holds and re-delivery is impossible. This also keeps
  behaviour stable if 1.0-final restores the abort arm.
  Date/author: 2026-08-16, planning agent.
- D5 — `discard` deletes the dead row (guarded, `RETURNING`, logged
  without payload).
  Rationale: the earlier marker-based sketch was self-contradictory — a
  discarded-but-`Killed` row would still match the dead predicate unless
  the listing grew a stringly-typed filter. Deletion is honest and bounds
  table growth; if retention of discarded jobs becomes a requirement it
  deserves its own decision, not a marker hack.
  Date/author: 2026-08-16, planning agent (panel finding).
- D6 — An additive partial index on the dead predicate is pre-authorized,
  created through the adapter setup path, applied only if EP-M3's
  `EXPLAIN` check shows a sequential scan.
  Rationale: `apalis.jobs` retains all history at these pins; an
  OR-predicate scan degrades with table growth. Conditional application
  keeps the footprint minimal and the evidence recorded.
  Date/author: 2026-08-16, planning agent (panel finding).
- D7 — One `JobRetryPolicy` per queue-adapter instance (per job family),
  no global constant.
  Rationale: the architecture document promises per-job-type queue
  configuration; per-instance policy state gives 5.3.1's per-queue workers
  a coherent shape without touching the `RouteQueue` port signature.
  Date/author: 2026-08-16, planning agent (panel finding).

## Outcomes & retrospective

To be completed at milestone boundaries and at closure. Before marking the
plan `COMPLETE`, reconcile the architecture document's "Background Job
Execution" prose with the implemented mechanism and confirm no upstream
assumption in `Conformance basis` was falsified.

## Revision note

2026-08-16: revised after the expert-panel design review. Material
changes: fatal dead-lettering is now an explicit storage-level action
(D4 rewritten; V5 strengthened to assert non-re-delivery); the two retry
loops (tower in-process versus storage refetch) are reconciled through the
named axiom AXIOM-ATTEMPT, an EP-M0 compile probe, and V4's
total-executions assertion; the domain policy is now the executed policy
via a delegating `tower::retry::Policy` (D1 rewritten); a
`TOTAL_BACKOFF_BUDGET` constructor constraint and obligation V1b bound
worker-slot occupancy and pin the orphan-recovery inequality; per-lease
attempt semantics, crash-loop risk, per-attempt logging, and the
unconfigured-orphan-recovery fact are documented; requeue and discard are
guarded compare-and-swap operations with provenance, `NotDead` errors,
and settled discard-as-delete semantics (D5); dead-letter and retry
metrics are named deliverables; `DeadJobId` newtype, `dead_at` naming,
`#[non_exhaustive]`, ordering, and payload-privacy contracts added to the
port; rc-coupling canaries (V8), attempt-mapping tests (V3b), the
optional partial index (D6), per-instance policy (D7), the operator
runbook, the harness promotion path, and the ADR split (ADR-002/ADR-003)
were added. Remaining work is unchanged in structure: EP-M0 through
EP-M4 pending approval.
