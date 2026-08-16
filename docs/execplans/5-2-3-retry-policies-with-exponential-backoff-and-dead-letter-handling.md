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
  dead-letter a job.
- A domain error classification for job execution failures (retryable versus
  fatal), following the precedent of
  `EnrichmentSourceError::is_retryable()`.
- A `DeadLetterQueue` driven port and a PostgreSQL adapter that lists and
  requeues dead jobs (rows in `apalis.jobs` that Apalis has marked `Killed`
  or that have exhausted their attempts).
- Adapter wiring that translates the domain policy into the Apalis 1.0-rc
  tower retry layer and persists the bounded `max_attempts` on every enqueued
  job, so the policy is enforced by the backend even before the 5.3.1 worker
  pools exist.

Observable outcomes:

- `cargo test -p backend` demonstrates: a job whose handler always fails is
  attempted exactly `max_attempts` times with increasing (capped, jittered)
  delays and is then recorded as dead; a fatal (non-retryable) failure is
  dead-lettered without further attempts; a dead job can be listed and
  requeued through the `DeadLetterQueue` port and is then eligible for
  processing again. These run against embedded PostgreSQL via `rstest-bdd`
  scenarios.
- Property tests pin the backoff invariants (bounded by cap, jitter within
  the configured fraction, monotone non-decreasing expected delay, retry
  decisions never exceed `max_attempts`).
- `make check-fmt`, `make typecheck`, `make lint`, and `make test` pass after
  each major milestone.
- Roadmap item 5.2.3 is marked done only after every gate passes and the
  documentation updates land.

## Context and orientation

Everything below is verifiable in the working tree; no prior plan is
required reading, though `docs/execplans/backend-5-2-2-job-structs-for-generate-route-and-enrichment.md`
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
  plan's policy attaches.
- Jobs: `backend/src/domain/jobs/generate_route.rs` and
  `backend/src/domain/jobs/enrichment.rs` define versioned V1 envelopes.
- Error-classification precedent:
  `backend/src/domain/ports/enrichment_source.rs` exposes
  `EnrichmentSourceError::is_retryable()`, and
  `backend/src/domain/overpass_enrichment_worker/attempt_error.rs` shows the
  house pattern for attempt-local error taxonomies.
- Metrics: the `RouteQueueMetrics` port with `RouteQueueOutcome`
  (`success`/`failure`) and the Prometheus adapter exist; this plan extends
  outcomes for retry/dead-letter observability only where cheap, deferring
  full queue-depth metrics to roadmap 6.x.
- No worker, `Monitor`, or `WorkerBuilder` exists anywhere in the crate.
  Production worker consumption is roadmap 5.3.1. Test-only consumption
  harnesses are in scope here because behavioural evidence of retry and
  dead-letter behaviour requires driving job execution in tests.

Facts about Apalis at the pinned versions (`apalis-core = "1.0.0-rc.7"`,
`apalis-postgres = "1.0.0-rc.6"`, `backend/Cargo.toml:34-35`), established
from docs.rs for those exact versions, the published crate sources, and the
apalis-dev repositories; these are external axioms for this plan:

- Retry middleware lives in the `apalis` facade crate
  (`apalis::layers::retry`, crate feature `retry`), not in `apalis-core`. It
  re-exports `tower::retry`: `RetryPolicy::retries(n)`,
  `.with_backoff(ExponentialBackoffMaker...)` for exponential backoff, and
  `.retry_if(pred)` for conditional retry. `apalis-core`'s
  `BackoffStrategy`/`BackoffConfig` govern backend polling cadence, not task
  retries. Adopting the upstream layer therefore requires adding the
  `apalis` facade crate as a dependency with the `retry` feature.
- The `apalis.jobs` row carries `status` (`Pending`, `Queued`, `Running`,
  `Done`, `Failed`, `Killed`), `attempts` (default 0), `max_attempts`
  (default 25), and `run_at`. The fetch function `apalis.get_jobs` selects
  `status = 'Pending' OR (status = 'Failed' AND attempts < max_attempts)`
  with `run_at < now()`.
- On acknowledgement, `calculate_status` maps `Ok` to `Done`, and an error
  with `attempt >= max_attempts` to `Killed`, otherwise `Failed`. The
  `Error::Abort => Killed` arm is present but commented out in
  apalis-postgres 1.0.0-rc.6, so abort-style fatal errors are not persisted
  as `Killed` until attempts are exhausted. This plan works around that gap
  (see Decision log D4).
- There is no built-in dead-letter table or requeue API. Dead jobs are
  `Killed` rows (or exhausted `Failed` rows) that remain in `apalis.jobs`;
  listing and requeueing require direct SQL.
- Backoff sleeps happen in-process inside the tower retry layer while the
  job row stays `Running` and locked; `run_at` is not rescheduled on
  failure. Attempts are written once at final acknowledgement. Long backoff
  therefore occupies a worker slot; orphan recovery
  (`reenqueue_orphaned_after`) is the safety net if a worker dies
  mid-backoff.

Terms used in this plan:

- "Dead-letter" a job: stop retrying it and park it in a queryable state
  (`Killed` in `apalis.jobs`) for operator inspection and optional requeue.
- "Fatal error": a job-execution failure that retrying cannot fix (for
  example, a payload that fails validation), classified so the job
  dead-letters immediately rather than burning attempts.
- "Retryable error": a transient failure (timeout, transport, rate limit)
  worth retrying with backoff.

## Constraints

Hard invariants. Violation requires escalation, not workarounds.

- Do not begin implementation before the user approves this plan.
- Keep scope to roadmap item 5.2.3. Do not implement 5.2.4 (trace
  propagation), 5.3.1 (worker pools, Kubernetes deployment, queue
  partitioning), or request-path dispatch. Production binaries must not gain
  a worker loop from this plan; consumption harnesses are test-only.
- Preserve hexagonal architecture. The retry policy and error classification
  live in the domain layer and must not import Apalis, tower, SQLx, Diesel,
  or Actix types. The `DeadLetterQueue` port is defined by the domain;
  the PostgreSQL adapter implements it in `backend/src/outbound/`.
  Background workers and caches interact with the domain exclusively via
  ports (roadmap section 5 preamble).
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
- Do not modify the Apalis-owned schema (`apalis.jobs`) shape and do not add
  Diesel migrations for Apalis tables. The dead-letter adapter works against
  the schema Apalis creates at runtime. Introducing a separate dead-letter
  table is out of scope (see Decision log D3).
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
- Upstream surprises: if the Apalis rc APIs differ materially from the
  axioms in "Context and orientation" (for example, the retry layer cannot
  be constructed against `apalis-postgres` 1.0.0-rc.6, or `max_attempts`
  cannot be set per enqueued task), stop, record the discovery, and escalate
  with options rather than upgrading pins.
- Iterations: if a red test still fails after three green attempts, or a
  gate fails three consecutive runs on the same cause, stop and escalate.
- Ambiguity: if retry semantics interact with idempotency in a way not
  settled here (for example, requeue resetting `attempts` proves unsafe for
  a job class), stop and present options.

## Risks

- Risk: Apalis 1.0-rc APIs are release candidates and may not match docs.rs
  exactly at compile time.
  Severity: medium. Likelihood: medium.
  Mitigation: milestone 1 starts with a compile-probe spike inside the test
  tree; the upstream-surprises tolerance catches divergence early.
- Risk: in-process backoff holds a job locked and occupies a worker slot for
  the whole retry chain; large caps could starve future workers.
  Severity: medium. Likelihood: medium.
  Mitigation: conservative default policy (small attempt count, capped
  delays, documented totals); ADR records the trade-off and flags run_at
  rescheduling as a 5.3.1+ follow-up (Decision log D2).
- Risk: the commented-out abort-to-`Killed` arm in apalis-postgres rc.6
  means fatal errors cannot rely on Apalis persisting `Killed` immediately.
  Severity: medium. Likelihood: high (confirmed in source).
  Mitigation: the fatal path is enforced by our policy layer setting the
  attempt budget, and the dead-letter adapter's queries treat exhausted
  `Failed` rows as dead alongside `Killed` rows (Decision log D4).
- Risk: behavioural tests need test-only job consumption, which could drift
  into building 5.3.1's worker early.
  Severity: low. Likelihood: medium.
  Mitigation: the harness lives under `backend/tests/support/`, is not
  exported from the crate, and the constraint section forbids production
  worker loops.
- Risk: `rstest-bdd` scenarios with embedded PostgreSQL and real sleeps
  could be slow or flaky.
  Severity: medium. Likelihood: medium.
  Mitigation: test policies use millisecond-scale delays; assertions check
  ordering and counts, not wall-clock precision; embedded-PostgreSQL setup
  reuses the provisioning helpers in
  `backend/tests/support/embedded_postgres.rs`.

## Skills and reference documents

Skills to load when implementing: `leta`, `rust-router` (then
`rust-errors` for the error taxonomy and `rust-async-and-concurrency` for
the tower layer work), `hexagonal-architecture`, `proptest`,
`rust-unit-testing`, `commit-message`, `pr-creation`, `en-gb-oxendict`.

Repository documents to read first:

- `docs/wildside-backend-architecture.md` — "Background Job Execution
  (Apalis Workers)" section (retry/dead-letter design intent, scope split
  between 5.2.1–5.2.4 and 5.3.1, the `decode_job` seam).
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
- `docs/documentation-style-guide.md` — ADR template and style rules.

## Conformance basis

Upstream artefacts (no Terms of Reference document exists for this
repository; the roadmap and architecture document are the governing
artefacts):

- `docs/backend-roadmap.md` item 5.2.3 (identifier ROAD-5.2.3) and the
  section 5 preamble constraint that workers interact with the domain only
  via ports (ROAD-5-PORTS).
- `docs/wildside-backend-architecture.md`, "Background Job Execution
  (Apalis Workers)" (identifier ARCH-JOBS): bounded retries with
  exponential backoff, dead-letter after max attempts, and the
  5.2.3/5.3.1 scope split.
- `docs/execplans/backend-5-2-2-job-structs-for-generate-route-and-enrichment.md`
  scope reservations (EP-522-SCOPE): job envelope stability, deferred
  retry/dead-letter ownership.
- New ADR produced by this plan: `docs/adr-002-apalis-retry-and-dead-letter-policy.md`
  (ADR-002).

Trace links:

```plaintext
ROAD-5.2.3 -> ARCH-JOBS -> EP-M1 -> backend/src/domain/jobs/retry.rs (policy + proptest invariants)
ROAD-5.2.3 -> ARCH-JOBS -> EP-M2 -> apalis retry wiring + max_attempts persisted (BDD: retry_policy.feature)
ROAD-5.2.3 -> ARCH-JOBS -> EP-M3 -> DeadLetterQueue port + PostgreSQL adapter (BDD: dead_letter.feature)
ROAD-5-PORTS -> EP-M1..M3 -> domain imports audit (no adapter types in domain)
EP-522-SCOPE -> EP-M2 -> insta snapshots unchanged for V1 envelopes
ADR-002 -> EP-M4 -> docs updates + contents.md registration
```

## Interfaces and dependencies

Domain layer (new module `backend/src/domain/jobs/retry.rs`, re-exported
from `backend/src/domain/jobs/mod.rs`; names indicative, refine during
implementation without changing intent):

```rust
/// Why a job execution attempt failed, as classified by the handler.
pub enum JobErrorKind {
    /// Transient failure; retrying with backoff may succeed.
    Retryable,
    /// Permanent failure; retrying cannot succeed.
    Fatal,
}

/// Bounded exponential backoff policy for background jobs.
pub struct JobRetryPolicy {
    max_attempts: NonZeroU32,
    base_delay: Duration,
    multiplier: f64,
    max_delay: Duration,
    jitter_fraction: f64, // 0.0..=1.0, applied as +/- fraction of the delay
}

pub enum RetryDecision {
    /// Retry after the given delay.
    Retry { delay: Duration },
    /// Stop retrying; the job is dead-lettered.
    DeadLetter,
}

impl JobRetryPolicy {
    pub fn decide(&self, attempt: NonZeroU32, kind: JobErrorKind, jitter: f64) -> RetryDecision;
    pub fn backoff_delay(&self, attempt: NonZeroU32, jitter: f64) -> Duration;
    pub fn max_attempts(&self) -> NonZeroU32;
}
```

`jitter` is passed in as a unit-interval sample so the policy stays pure and
deterministic under test; the adapter supplies randomness.

Domain port (new file `backend/src/domain/ports/dead_letter_queue.rs`, error
via `define_port_error!`):

```rust
/// A job parked after exhausting retries or failing fatally.
pub struct DeadJob {
    pub id: String,
    pub job_type: String,
    pub attempts: u32,
    pub last_result: Option<String>,
    pub done_at: Option<DateTime<Utc>>,
}

#[async_trait]
pub trait DeadLetterQueue: Send + Sync {
    async fn list_dead(&self, limit: u32) -> Result<Vec<DeadJob>, DeadLetterError>;
    async fn requeue(&self, job_id: &str) -> Result<(), DeadLetterError>;
    async fn discard(&self, job_id: &str) -> Result<(), DeadLetterError>;
}
```

Outbound adapter (new file
`backend/src/outbound/queue/postgres_dead_letter.rs`):
`PostgresDeadLetterQueue` holding an `sqlx::PgPool`, querying `apalis.jobs`
for `status = 'Killed' OR (status = 'Failed' AND attempts >= max_attempts)`;
`requeue` resets `status = 'Pending', attempts = 0, run_at = now()`;
`discard` sets `status = 'Killed'` with a marker in `last_result` (or
deletes; settle in M3 and record in the Decision log).

Outbound wiring (extend `backend/src/outbound/queue/`, for example
`retry_layer.rs`): a factory translating `JobRetryPolicy` into the Apalis
tower stack (`RetryPolicy::retries(...)`, exponential backoff maker
configured from the policy, `retry_if` honouring `JobErrorKind`), plus
enqueue-side persistence of `max_attempts` on the task context so bounded
retries hold even under Apalis defaults. Production dependency addition:

```toml
apalis = { version = "1.0.0-rc.7", default-features = false, features = ["retry"] }
```

Dev-dependency addition: `googletest = "0.13"` (or current compatible
release) for matcher-based assertions alongside `pretty_assertions`.

## Verification plan

External axioms (not verified here; treated as third-party contract):
tower's `ExponentialBackoffMaker` produces delays per its documentation;
Apalis persists `attempts`/`status` per the ack path described in "Context
and orientation"; embedded PostgreSQL behaves like production PostgreSQL for
the SQL used. Repository-owned logic that builds on these interfaces is
verified against the real interface (embedded PostgreSQL, real Apalis
storage) at the behavioural tier.

Obligations:

- Obligation V1 (backoff bounded): for every attempt `a` in
  `1..=max_attempts` and jitter sample `j` in `[0, 1]`,
  `backoff_delay(a, j) <= max_delay * (1 + jitter_fraction)` and
  `>= 0`.
  Method: property test (`proptest`) over generated policies, attempts, and
  jitter samples.
  Rationale: the invariant spans a continuous input domain; examples cannot
  cover it.
  Artefact: `backend/src/domain/jobs/retry/tests/properties.rs` (or inline
  `tests` module, matching house layout).
  Evidence: `cargo test -p backend retry` — fails before implementation
  (red), passes after.
  Non-vacuity: generators emit policies across base delays from 1 ms to
  minutes, multipliers 1.0–10.0, and jitter 0.0–1.0; a seeded fault
  (temporarily removing the cap clamp) must make the property fail.
- Obligation V2 (monotone growth pre-cap): with jitter fixed at the
  midpoint, `backoff_delay(a+1) >= backoff_delay(a)` until the cap is
  reached.
  Method: property test plus parameterized `rstest` cases at the boundary
  (attempt where the cap first binds).
  Non-vacuity: generator classification records the fraction of cases where
  the cap binds; both classes (capped, uncapped) must be exercised; negative
  control: inverting the multiplier must fail the property.
- Obligation V3 (decision totality and bounds): `decide` returns
  `DeadLetter` whenever `kind == Fatal` or `attempt >= max_attempts`, and
  `Retry` otherwise; no path yields a retry beyond `max_attempts`.
  Method: exhaustive parameterized tests over the finite classification
  (`Fatal`/`Retryable` × attempt below/at/above the bound) plus a property
  test over attempt ranges. `googletest` matchers for the decision shape,
  `pretty_assertions` for equality.
  Rationale: the decision function is a small finite state map; exhaustive
  partitioning plus properties is proportionate. No bounded model checking
  (`kani`) or proof (`verus`) is warranted: the logic has no unsafe code, no
  unbounded state, and no lemma beyond the enumerated partitions — this is
  recorded as the explicit justification required when declining heavier
  rigour.
  Non-vacuity: each partition has a witness case; a seeded off-by-one fault
  in the bound comparison must be rejected.
- Obligation V4 (persisted retry bound): a job enqueued through the adapter
  carries the policy's `max_attempts` in `apalis.jobs.max_attempts`, and a
  handler that always fails is attempted exactly `max_attempts` times before
  the row becomes dead (status `Killed`, or `Failed` with exhausted
  attempts).
  Method: behavioural test (`rstest-bdd`) against embedded PostgreSQL with a
  test-only consumption harness; assertions on the row's `attempts` and
  `status` columns.
  Non-vacuity: the scenario also asserts at least one intermediate retry
  occurred (attempt counter observed by the failing handler > 1), so a
  policy that never retries fails the scenario; a control scenario with a
  succeeding handler must end `Done` with one attempt.
- Obligation V5 (fatal short-circuit): a handler returning a fatal error
  dead-letters the job without exhausting the attempt budget.
  Method: behavioural test as V4 with a fatal-classified failure; assert the
  handler ran exactly once.
  Non-vacuity: contrast with V4's retryable scenario in the same feature
  file; misclassifying fatal as retryable makes the run-count assertion
  fail.
- Obligation V6 (dead-letter round trip): a dead job appears in
  `DeadLetterQueue::list_dead`, `requeue` makes it eligible again (fetched
  and re-executed), and `discard` removes it from eligibility and listing
  semantics as settled in M3.
  Method: behavioural test (`rstest-bdd`) with embedded PostgreSQL.
  Non-vacuity: listing before any failure must be empty (witness that the
  query filters live jobs); requeue is proven by observing a subsequent
  successful execution, not merely a status flip.
- Obligation V7 (snapshot stability): V1 job envelope snapshots from 5.2.2
  are byte-identical after this plan; any new operator-facing rendering of
  dead jobs (if introduced) is snapshot-pinned with `insta`.
  Method: existing `insta` suites re-run; new snapshots only for new
  multivariant output.
  Non-vacuity: `cargo insta test` fails on any drift by construction.

If implementation reveals an invariant not listed here (for example, an
ordering constraint between requeue and concurrent fetch), return to this
section and extend it before continuing.

## Plan of work

### Milestone EP-M0 — approval and baseline audit (no code)

Confirm plan approval. Run `scrutineer` for a baseline gate run on the
untouched branch and record results in `Progress` (known issue: local
Whitaker lint suite may be ahead of CI's pin; record, do not chase).
Re-verify the Apalis axioms compile-time cheaply: add nothing yet; simply
`cargo tree -p backend | grep apalis` and note versions.

Plateau: clean tree, recorded baseline. Recovery: none needed.

### Milestone EP-M1 — domain retry policy and classification (red-green)

Red: add failing `rstest` unit tests, `googletest`/`pretty_assertions`
assertions, and `proptest` properties for `JobRetryPolicy`,
`RetryDecision`, and `JobErrorKind` (obligations V1–V3). Add `googletest`
to `[dev-dependencies]`.

Green: implement `backend/src/domain/jobs/retry.rs` (pure; no adapter
imports). Constructor validates configuration (non-zero attempts,
multiplier >= 1.0, jitter in `[0, 1]`) returning a build error consistent
with `EnrichmentJobBuildError`'s style.

Refactor: documentation comments with examples per house style; keep the
file under 400 lines.

Plateau: domain policy fully tested and inert (nothing consumes it yet —
coherent because it is a pure library addition). Gates via `scrutineer`;
commit.

### Milestone EP-M2 — Apalis retry wiring and bounded attempts

Red: add the BDD feature `backend/tests/features/queue_retry_policy.feature`
(embedded below) and step bindings in
`backend/tests/queue_retry_policy_bdd.rs`; add a test-only consumption
harness under `backend/tests/support/` (worker built from
`apalis` retry layer + storage, driven for a bounded number of polls).
Scenarios fail because no wiring exists.

Green: add the `apalis` facade dependency (feature `retry`); implement the
retry-layer factory and enqueue-side `max_attempts` persistence in
`backend/src/outbound/queue/`; classification maps `JobErrorKind::Fatal`
into the non-retryable path (`retry_if` predicate and, if usable at this
rc, `AbortError`).

Refactor: extend `RouteQueueOutcome`/metrics only if a retry outcome falls
out naturally; otherwise record deferral.

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
    Then the handler runs three times
    And the job is recorded as dead

  Scenario: A fatally failing job is dead-lettered immediately
    Given a queue with a retry policy of 3 attempts and millisecond backoff
    And a job whose handler fails with a fatal error
    When the worker processes the queue
    Then the handler runs once
    And the job is recorded as dead
```

Plateau: retry policy enforced end to end under test; production behaviour
unchanged except enqueued rows now carry the bounded `max_attempts`
(persisted-format note: this only tightens the existing column's default;
no migration). Gates; commit.

### Milestone EP-M3 — dead-letter port and PostgreSQL adapter

Red: add `backend/tests/features/queue_dead_letter.feature` (embedded
below) and step bindings in `backend/tests/queue_dead_letter_bdd.rs`; add
failing unit tests for the port error type and SQL-free domain pieces.

Green: define the `DeadLetterQueue` port; implement
`PostgresDeadLetterQueue`; settle discard semantics and record the choice
in the Decision log.

Feature specification:

```gherkin
Feature: Dead-letter listing and requeue

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

  Scenario: Listing dead jobs ignores live jobs
    Given a pending job and a running job exist
    When the operator lists dead jobs
    Then the listing is empty
```

Plateau: dead-letter handling usable by future operators/workers through a
domain port. Gates; commit.

### Milestone EP-M4 — documentation, ADR, roadmap closure

Write `docs/adr-002-apalis-retry-and-dead-letter-policy.md` per the
style-guide template recording decisions D1–D4. Update
`docs/wildside-backend-architecture.md` ("Background Job Execution") from
aspiration to implemented reality; update `docs/developers-guide.md`
("Apalis queue adapter boundaries" and test-infrastructure sections) with
the new symbols, dependency, and conventions; register the ADR in
`docs/contents.md`. `docs/users-guide.md` is expected to need no change
(queue internals are not client-visible); confirm and record in the
Decision log. Mark roadmap item 5.2.3 done with an execution note. Full
gates via `scrutineer` including Markdown lint and Mermaid validation;
commit; open the implementation pull request.

Plateau: feature complete, documented, roadmap closed.

## Milestones and plateaus — conformance checks

At each milestone boundary check: ROAD-5-PORTS holds (no adapter imports in
`backend/src/domain/`); ARCH-JOBS scope split intact (no production worker
loop); no unapproved dependency, public interface, or persisted-format
change beyond the `max_attempts` tightening named in EP-M2; V1 envelope
snapshots unchanged; trace links in `Conformance basis` current.

Compatibility decision: none required. All touched APIs are pre-1.0,
application-internal surfaces; no aliases, shims, or dual implementations
are permitted. The persisted format question is confined to
`apalis.jobs.max_attempts`, an existing column whose per-row value this
plan sets explicitly; deployed rows with the default 25 remain valid and
simply retain their larger budget.

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
acceptance behaviour; unit and property tests discharge V1–V3;
behavioural tests discharge V4–V6; `insta` re-runs discharge V7.

Quality criteria: all four Makefile gates pass; obligations V1–V7
discharged with non-vacuity evidence recorded in `Artefacts and notes`;
CodeRabbit review (if requested) has no unresolved in-scope findings;
roadmap 5.2.3 ticked with an execution note.

## Idempotence and recovery

All steps are additive and re-runnable. Embedded PostgreSQL tests provision
throwaway clusters; re-running is safe. If a milestone fails mid-way,
`git status` and the committed plateau of the previous milestone define the
recovery point; revert uncommitted changes rather than patching forward. No
destructive operations exist in this plan.

## Progress

- [x] (2026-08-16) Reconnaissance completed: code recon, documentation
  recon, and Apalis 1.0-rc capability research recorded in this plan.
- [x] (2026-08-16) Plan drafted and reviewed by the expert panel; revisions
  applied.
- [ ] EP-M0 approval and baseline audit.
- [ ] EP-M1 domain retry policy and classification.
- [ ] EP-M2 Apalis retry wiring and bounded attempts.
- [ ] EP-M3 dead-letter port and PostgreSQL adapter.
- [ ] EP-M4 documentation, ADR, roadmap closure.

## Surprises & discoveries

- Observation: apalis-postgres 1.0.0-rc.6 does not persist abort errors as
  `Killed` (the `calculate_status` arm is commented out upstream).
  Evidence: published crate source for rc.6.
  Impact: fatal-error dead-lettering must not rely on Apalis writing
  `Killed` on abort; policy and dead-letter queries compensate (D4).
- Observation: the Apalis retry layer performs backoff in-process while the
  job row stays locked; `run_at` is not rescheduled on failure.
  Evidence: `ack.sql` in the published rc.6 crate updates only
  status/attempts/last_result/done_at; tower `Retry` awaits the backoff
  future within the same execution.
  Impact: delays must stay small; persisted-schedule backoff is out of
  scope and recorded as a known limitation (D2).

## Decision log

- D1 — Adopt the upstream Apalis tower retry layer rather than a bespoke
  retry loop.
  Rationale: it is the supported mechanism at these pins, integrates with
  the worker builder 5.3.1 will use, and keeps our code to policy
  translation. A bespoke loop would duplicate tower and complicate 5.3.1.
  Date/author: 2026-08-16, planning agent (subject to user approval of this
  plan).
- D2 — Accept in-process backoff with conservative caps; defer persisted
  (`run_at`-based) backoff.
  Rationale: rc.6 provides no failure-time `run_at` rescheduling; building
  one means custom ack middleware, which belongs with 5.3.1's worker
  ownership if ever. Recorded in ADR-002 with the worker-slot trade-off.
  Date/author: 2026-08-16, planning agent.
- D3 — Dead letters are rows in `apalis.jobs` (status `Killed` or exhausted
  `Failed`), surfaced through a domain `DeadLetterQueue` port; no separate
  table.
  Rationale: matches Apalis's model and graphile-worker prior art; avoids
  schema divergence from the library-managed tables; the port isolates the
  representation so a future table swap stays adapter-local.
  Date/author: 2026-08-16, planning agent.
- D4 — Fatal errors short-circuit via policy classification, not via
  Apalis abort persistence.
  Rationale: the abort-to-`Killed` persistence path is incomplete at rc.6;
  relying on it would make behaviour version-dependent. Our classification
  plus attempt bounds gives deterministic dead-lettering either way.
  Date/author: 2026-08-16, planning agent.

## Outcomes & retrospective

To be completed at milestone boundaries and at closure. Before marking the
plan `COMPLETE`, reconcile the architecture document's "Background Job
Execution" prose with the implemented mechanism and confirm no upstream
assumption in `Conformance basis` was falsified.
