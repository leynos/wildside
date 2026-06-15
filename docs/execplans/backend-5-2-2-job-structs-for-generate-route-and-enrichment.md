# Define domain job structs `GenerateRouteJob` and `EnrichmentJob` (backend 5.2.2)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: APPROVED / IN PROGRESS

This plan covers roadmap item 5.2.2 only:

> Define job structs for `GenerateRouteJob` and `EnrichmentJob`.

No implementation work may begin until this plan is explicitly approved.
Approval authorizes only the milestones below; it does not authorize wiring
the new structs into request-path dispatch, worker bootstrap, retry policy,
trace propagation, queue partitioning, or any later roadmap item.

## Purpose / big picture

Wildside's queue adapter (roadmap item 5.2.1) currently accepts an opaque
`serde_json::Value` plan payload. Future worker consumers, retry policy
(roadmap 5.2.3), trace propagation (5.2.4), and worker deployment (5.3.1) all
need a stable, versioned, and idempotent contract for the jobs that flow
through the queue. This plan adds that contract at the domain layer, so
inbound adapters (HTTP, future schedulers) can build typed jobs and worker
handlers can decode them deterministically, without leaking Apalis, SQLx, or
PostgreSQL details into the domain.

The observable outcome of this plan is:

- New domain types `backend::domain::jobs::GenerateRouteJob` and
  `backend::domain::jobs::EnrichmentJob` exist and round-trip through
  `serde_json` and through the existing `RouteQueue` port using a typed
  `Plan` associated type.
- The JSON shape of the V1 (version 1) payload is pinned by `insta`
  (Immediate Snapshot Test Assistant) snapshots so future versions cannot
  drift silently.
- The types are tested through unit tests (`rstest`), property-based
  round-trip tests (`proptest`), and behavioural tests (`rstest-bdd`)
  exercising enqueue via the existing `StubRouteQueue` and (where embedded
  PostgreSQL is available) `ApalisRouteQueue`.
- `make check-fmt`, `make lint`, and `make test` pass after each major
  milestone.
- `coderabbit review --agent` has no unresolved in-scope concerns at the end
  of each major milestone.
- The roadmap item is marked done only after every gate above is clean.

## Constraints

Hard invariants. Violation requires escalation, not workarounds.

- Implementation was explicitly approved by the user on 2026-06-14. Keep
  scope to the milestones in this plan.
- Keep scope to backend roadmap item 5.2.2. Do not implement or mark done
  5.2.3 (retry policy and dead-letter handling), 5.2.4 (trace identifier
  propagation), 5.3.1 (worker deployment), or any route-submission dispatch
  item. The `TODO(#276)` markers in
  `backend/src/domain/route_submission/mod.rs:241` and
  `backend/src/domain/route_submission/mod.rs:295` remain in place.
- Preserve hexagonal architecture. The new job structs live in the domain
  layer (`backend/src/domain/jobs/*`). They must not import Apalis, SQLx,
  Diesel, Actix, or any outbound adapter type.
- Do not change the `RouteQueue` trait signature or the `JobDispatchError`
  variants defined in `backend/src/domain/ports/route_queue.rs`.
- Keep the public `RouteSubmissionRequest::payload: serde_json::Value`
  contract unchanged
  (`backend/src/domain/ports/route_submission.rs:17`). Adding a typed
  conversion from a submission to a `GenerateRouteJob` is permitted as a
  domain-layer helper; rewiring the submission API surface is not.
- Do not modify `backend/src/outbound/queue/apalis_route_queue.rs` beyond
  what is strictly required to demonstrate that the existing adapter accepts
  the new typed `Plan`. Specifically, do not switch from
  `PostgresStorage<serde_json::Value>` to `SharedPostgresStorage` or to a
  typed `PostgresStorage<GenerateRouteJob>` in this milestone. Storage shape
  is queue-adapter territory and belongs with 5.3.1.
- Do not upgrade `apalis-core` (`backend/Cargo.toml:34`),
  `apalis-postgres` (`backend/Cargo.toml:35`), or `sqlx` as part of this
  plan. Pin moves are out of scope.
- Do not introduce trace-identifier fields on the V1 job structs. Trace
  propagation belongs to roadmap 5.2.4; reserving the field now would
  invite half-finished plumbing. The `#[serde(tag = "v")]` envelope keeps
  the door open for a V2 schema that adds trace metadata cleanly.
- Do not derive `Serialize` or `Deserialize` on
  `backend::domain::trace_id::TraceId`
  (`backend/src/domain/trace_id.rs:34`). That domain primitive is
  deliberately not serde-derived today; revising it is 5.2.4's job.
- Keep documentation in en-GB-oxendict style and follow
  `docs/documentation-style-guide.md`. Wrap paragraphs at 80 columns; wrap
  code at 120 columns. Use sentence case for headings.
- Keep source files below 400 lines, splitting modules before completing a
  milestone if necessary.
- Prefer Makefile targets over raw tool invocations for final gate runs.
- Run tests, formatting, and linting sequentially. Do not run them in
  parallel because the repository relies on build caches.
- Capture long command output with `tee` under `/tmp` using the
  `/tmp/$ACTION-wildside-$(git branch --show-current).out` template.
- Commit each approved milestone only after its gate passes. Do not amend
  earlier commits; create new ones if a gate fails.
- Do not mark roadmap item 5.2.2 done until every gate and every CodeRabbit
  review pass cleanly.

## Tolerances (exception triggers)

- Scope budget: stop and escalate if satisfying 5.2.2 requires changing more
  than ten production source files or more than 600 net non-test lines
  outside tests, fixtures, and documentation. Tests and snapshots may be as
  large as they need to be.
- Port shape: stop and escalate before changing the `RouteQueue` trait, the
  `JobDispatchError` enum, or the `RouteSubmissionRequest`/`Response`
  structs.
- Queue adapter: stop and escalate before touching
  `ApalisPostgresProvider`, `PostgresStorage`, or `QueueProvider`.
- Dependencies: stop and escalate before adding any production dependency
  not already in `backend/Cargo.toml`. The plan expects only dev-dependency
  additions of `pretty_assertions` and `googletest` (both are currently
  absent from the workspace; see Surprises & Discoveries) plus continued
  use of the existing `rstest`, `rstest-bdd`, `proptest`, and `insta` crates.
- Iterations: stop and document logs if any gate (`make check-fmt`,
  `make lint`, `make test`, or the focused queue/job suites) still fails
  after three focused repair loops.
- CodeRabbit: stop and document the concern if `coderabbit review --agent`
  reports a finding that would require widening the approved scope (for
  example, asking to wire dispatch or trace propagation).
- Time: stop and re-plan if a milestone takes longer than four working
  hours of agent time.
- Ambiguity: stop and present options if a single choice between two
  observable contracts (for example, a tagged versioning envelope versus a
  flat schema with a `schema_version` field) materially changes the
  resulting public surface.

## Risks

Each risk lists severity, likelihood, and the mitigation that keeps it
contained within this plan.

- Risk: V1 schema is wrong on first publication and we have to break it
  before any consumer ships.
  Severity: medium. Likelihood: low.
  Mitigation: pin the V1 JSON shape under an `insta` snapshot from the
  first commit; review the snapshot during the Logisphere expert pass
  before merging. The `#[serde(tag = "v")]` envelope means a V2 can be
  added without breaking V1 consumers.

- Risk: agents implementing later milestones (5.2.3 retry policy, 5.2.4
  trace propagation, 5.3.1 worker deployment) misread the V1 schema as
  final and skip the envelope.
  Severity: medium. Likelihood: medium.
  Mitigation: explicitly document the versioning policy in
  `docs/wildside-backend-architecture.md` and
  `docs/developers-guide.md`, and place a short header doc-comment on the
  envelope enum referencing the policy.

- Risk: coupling `GenerateRouteJob::V1` directly to the current
  `RouteRequest` HTTP body shape
  (`backend/src/inbound/http/routes.rs:25`) bakes inbound-layer choices
  into the domain.
  Severity: high. Likelihood: medium.
  Mitigation: define the job's `origin`, `destination`, and `preferences`
  fields as domain-shaped `serde_json::Value` for now, matching the
  current submission API. Add a conversion `TryFrom<&
  RouteSubmissionRequest>` so the boundary is one explicit, testable seam
  rather than implicit alignment. When the route engine grows typed
  inputs (`wildside-engine`), a later plan can tighten this without
  rewriting the queue.

- Risk: `apalis-core` 1.0.0-rc.7 lacks the first-class idempotency feature
  added in rc.8, so the job struct must carry its own idempotency key.
  Severity: low. Likelihood: high.
  Mitigation: include an explicit `idempotency_key: Option<IdempotencyKey>`
  on each V1 struct. When the workspace later upgrades to rc.8 or newer
  and adopts the framework-native key, the existing field becomes the
  source of truth that maps into `TaskBuilder::id(...)`.

- Risk: the `EnrichmentJob` bounding box is a primitive `[f64; 4]` and
  accepts geometrically invalid inputs (out-of-range coordinates, inverted
  ordering, antimeridian wrap).
  Severity: medium. Likelihood: medium.
  Mitigation: introduce a `BoundingBox` newtype in the same module, with
  validating constructors, `serde` round-trip, and a `proptest` strategy.
  Reject inversions and out-of-range coordinates at construction time. The
  V1 contract explicitly does not support antimeridian-wrapped boxes (see
  Decision Log); wrapping callers must split the box client-side, and the
  rejection error names the policy so the failure is self-describing.

- Risk: `EnrichmentJobV1::tags` is an unbounded `Vec<String>`. A misuse
  could push tens of thousands of strings into the queue table and bloat
  `apalis.jobs` rows.
  Severity: medium. Likelihood: low.
  Mitigation: bound the vector at construction time (`max_tags = 64`,
  `max_tag_length = 64`) with a dedicated error variant, and add a
  `proptest` case that rejects oversized inputs.

- Risk: future agents add a new optional field to V1 without cutting V2,
  silently breaking older workers running with `deny_unknown_fields`.
  Severity: high. Likelihood: medium.
  Mitigation: place a short evolution rule in the V1 type doc-comment
  ("additive changes require a `V2` variant; do not relax
  `deny_unknown_fields`") and repeat it in the developers guide section
  added during milestone M5. Snapshot regeneration etiquette in the
  developers guide explicitly calls out that a snapshot diff implies a
  V2 cut, not a V1 edit.

- Risk: a worker pod loaded with V1 code receives a `v: "2"` envelope
  after a future schema bump and panics or silently drops the job.
  Severity: high. Likelihood: medium.
  Mitigation: document the worker-side policy in this plan and in
  `docs/wildside-backend-architecture.md`: an unknown envelope variant
  is a `JobDispatchError::Rejected` outcome — fail the job, log loudly,
  and route to dead-letter when retry policy (5.2.3) lands. The
  implementation of that policy is out of scope; the contract for it is
  in scope here.

- Risk: the existing Overpass enrichment worker uses
  `OverpassEnrichmentRequest`
  (`backend/src/domain/ports/overpass_enrichment_source.rs:14`), and a
  second domain type for the same logical input invites drift.
  Severity: low. Likelihood: medium.
  Mitigation: provide a deterministic `EnrichmentJob::to_overpass_request`
  helper plus a property test asserting that the conversion preserves the
  bounding box and tag list. Document the relationship in the architecture
  doc so the two types stay coordinated.

- Risk: `TraceId` (`backend/src/domain/trace_id.rs:34`) is not
  serde-derived, so anyone reading the architecture doc will expect the
  V1 envelope to include trace IDs and will be surprised when it does not.
  Severity: low. Likelihood: medium.
  Mitigation: state the omission explicitly in the architecture doc,
  cross-reference roadmap 5.2.4, and add a Decision Log entry.

- Risk: `googletest` and `pretty_assertions` are absent from the workspace
  today, yet the task instructions require their assertions.
  Severity: low. Likelihood: high.
  Mitigation: add both as workspace `dev-dependencies` in milestone M1,
  scoped to job-struct test modules, and confirm the addition with the
  user during the approval pass. If approval requires keeping them out,
  fall back to `assert_eq!` plus structured failure messages.

## Skills and reference documents

Load the following skills while implementing this plan:

- `leta`: navigate symbols and references before editing code.
- `rust-router`: route any Rust language issue to the smallest useful Rust
  skill.
- `hexagonal-architecture`: keep job structs domain-owned and adapter-free.
- `rust-types-and-apis`: shape the job envelope, newtypes, and conversions.
- `rust-errors`: design the build/validation errors for the job structs.
- `python-iterators-and-generators` is not relevant; do not load it.
- `proptest`: write the round-trip and bounding-box strategies.
- `commit-message`: produce file-based commit messages for each milestone.
- `pr-creation` and `en-gb-oxendict-style`: draft the pull request body.

Read these repository documents before implementation:

- `docs/backend-roadmap.md` (section 5.2 in particular).
- `docs/wildside-backend-architecture.md` (background job and trace
  sections).
- `docs/developers-guide.md` (queue adapter boundaries section).
- `docs/users-guide.md` (verify no end-user behaviour changes).
- `docs/documentation-style-guide.md`.
- `docs/rust-testing-with-rstest-fixtures.md`.
- `docs/rstest-bdd-users-guide.md`.
- `docs/rust-doctest-dry-guide.md`.
- `docs/complexity-antipatterns-and-refactoring-strategies.md`.
- `docs/pg-embed-setup-unpriv-users-guide.md` (only if a BDD scenario
  requires embedded PostgreSQL).
- `docs/execplans/backend-5-2-1-apalis-route-queue.md` (for the queue
  adapter context that 5.2.2 builds on).

External references confirmed during planning:

- `https://docs.rs/apalis-core/1.0.0-rc.7/apalis_core/` — confirms there is
  no `Job` trait in 1.0; task identity lives on the `Task<Args, Ctx, Id>`
  envelope and `Args` only needs to satisfy the codec
  (`Serialize + DeserializeOwned` for `JsonCodec`).
- `https://docs.rs/apalis-postgres/latest/apalis_postgres/` — confirms
  `PostgresStorage<Args>` defaults, and that metadata travels in
  `Parts`/`TaskBuilder` rather than the payload.
- `https://github.com/apalis-dev/apalis/blob/main/CHANGELOG.md` — confirms
  rc.8 added task idempotency and rc.9 adds SQL idempotency (#736); we
  therefore carry our own `idempotency_key` until the workspace upgrades.

## Current repository orientation

The branch is
`backend-5-2-2-job-structs-for-generate-route-and-enrichment`,
tracking `origin/backend-5-2-2-job-structs-for-generate-route-and-enrichment`
(to be pushed during milestone M5). The relevant files today are:

- `backend/src/domain/ports/route_queue.rs:17` defines `RouteQueue` with a
  generic associated type `Plan: Send + Sync` and the
  `JobDispatchError::{Unavailable, Rejected}` enum.
- `backend/src/outbound/queue/apalis_route_queue.rs:171` exports
  `pub type ApalisRouteQueue<P> = GenericApalisRouteQueue<P, ApalisPostgresProvider>;`
  parameterised over `P: Serialize + Send + Sync`. The production storage is
  still `PostgresStorage<serde_json::Value>` and the typed `P` is serialised
  at enqueue time.
- `backend/src/outbound/queue/stub_route_queue.rs:31` exports
  `StubRouteQueue<P>` which discards plans. This is the cheap test seam for
  job-struct integration without embedded PostgreSQL.
- `backend/src/domain/ports/route_submission.rs:17` defines
  `RouteSubmissionRequest { idempotency_key, user_id, payload }` with
  `payload: serde_json::Value`.
- `backend/src/inbound/http/routes.rs:25` defines the inbound HTTP
  `RouteRequest { origin, destination, preferences }` body shape.
- `backend/src/domain/ports/overpass_enrichment_source.rs:14` defines
  `OverpassEnrichmentRequest { job_id, bounding_box, tags }`.
- `backend/src/domain/idempotency/key.rs:35` defines `IdempotencyKey`
  (UUID-backed, serde-derived).
- `backend/src/domain/user.rs:74` defines `UserId` (serde-derived).
- `backend/src/domain/trace_id.rs:34` defines `TraceId` without serde
  derives. Roadmap 5.2.4 owns adding those derives.
- `backend/src/domain/mod.rs` re-exports the domain primitives. The plan
  adds a new `pub mod jobs;` declaration here.

Key terms used in this plan:

- A job struct is a plain Rust type that names the data a worker needs to
  execute one unit of background work. In this plan it is also the
  associated `Plan` type fed into `RouteQueue::enqueue`.
- A versioning envelope is the outermost enum that carries a `v` tag in
  serialised form so old payloads remain decodable when new shapes are
  introduced.
- A driven port is a domain-owned trait that adapters implement to provide
  infrastructure. `RouteQueue` is the relevant driven port here.

## Plan of work

Work in five milestones, each ending with gates and a CodeRabbit pass. Do
not move on until the milestone's gate is green.

### Milestone 0: approval and baseline audit

Once this plan is explicitly approved, confirm orientation:

```bash
git branch --show-current
git status --short --branch
leta workspace add "$(pwd)"
leta grep "RouteQueue|StubRouteQueue|GenericApalisRouteQueue|JobDispatchError" \
  backend -k trait,struct,enum,function,method --head 100
leta grep "GenerateRouteJob|EnrichmentJob" backend -k struct,enum --head 50
```

Record any drift from the orientation paragraph above in
`Surprises & Discoveries`. If the audit shows the structs already exist or
the queue adapter has shifted under the plan, stop and escalate.

### Milestone 1: scaffold `domain::jobs` module and dev-dependencies

Add a new domain submodule and prepare the test surface:

1. Add `pub mod jobs;` to `backend/src/domain/mod.rs` and create
   `backend/src/domain/jobs/mod.rs` with the public re-exports listed in
   "Interfaces and dependencies" below. The module starts empty apart from
   the documentation header and `pub mod generate_route;` /
   `pub mod enrichment;` declarations.
2. Add `pretty_assertions = "1"` and `googletest = "0.13"` (or the latest
   1.x line that compiles with edition 2024) to
   `backend/Cargo.toml`'s `[dev-dependencies]`. If either crate fails to
   compile, stop and escalate; do not silently downgrade test rigour.
3. Create empty test module files
   `backend/src/domain/jobs/generate_route.rs` and
   `backend/src/domain/jobs/enrichment.rs`, each with a
   `#[cfg(test)] mod tests;` declaration and matching `tests.rs` siblings
   so subsequent milestones land tests next to types. Snapshots live in
   `backend/src/domain/jobs/snapshots/`.
4. Run only the affected unit suite to confirm the scaffold compiles:

   ```bash
   set -o pipefail
   cargo check -p backend 2>&1 \
     | tee /tmp/check-wildside-backend-5-2-2-scaffold.out
   ```

5. Run `coderabbit review --agent`. Resolve in-scope findings or document
   them under `Surprises & Discoveries`.

Commit the scaffold with a file-based commit message before continuing.

### Milestone 2: define `GenerateRouteJob`

Implement `GenerateRouteJob` in
`backend/src/domain/jobs/generate_route.rs`. The minimum bar:

1. Define the versioning envelope and V1 payload using the signatures in
   "Interfaces and dependencies" below. Derive
   `Clone, Debug, PartialEq, Eq, Serialize, Deserialize`. Use
   `#[serde(deny_unknown_fields)]` on V1 to reject unknown keys and
   `#[serde(rename_all = "camelCase")]` to match the rest of the public
   contract.
2. Add a `GenerateRouteJob::v1(...)` constructor and a fallible helper
   `GenerateRouteJob::try_from_submission(&RouteSubmissionRequest,
   request_id, enqueued_at)` that returns
   `Result<Self, GenerateRouteJobBuildError>`.
   The helper validates that `payload` is a JSON object containing
   `origin` and `destination`, and copies the optional `preferences` field
   if present. Failure cases map to `GenerateRouteJobBuildError` variants
   (use the existing `define_port_error!` macro pattern; see
   `backend/src/domain/ports/route_queue.rs:6` and
   `backend/src/domain/ports/overpass_enrichment_source.rs:50` for the
   established style).
3. Add `rstest` unit tests covering:
   - Constructor accepts a well-formed submission.
   - Constructor rejects payloads that are not objects.
   - Constructor rejects payloads missing `origin` or `destination`.
   - Round-trip through `serde_json::to_value` and back is the identity.
   - Unknown fields are rejected on decode (uses `deny_unknown_fields`).
4. Add a `proptest` strategy in
   `backend/src/domain/jobs/generate_route/proptest_strategies.rs` (or
   inline in the tests module if it stays under 400 lines) that generates
   semi-realistic V1 payloads and proves
   `parse(serialize(job)) == Ok(job)` for every generated value. Use a
   bounded strategy so shrinking remains tractable.
5. Add an `insta` snapshot test that locks the V1 JSON shape for a
   canonical fixture (use `Uuid::nil()` and a known `DateTime<Utc>`). The
   snapshot lives under `backend/src/domain/jobs/snapshots/`. Review the
   first snapshot manually before approving it.
6. Run the focused suite:

   ```bash
   set -o pipefail
   cargo test -p backend domain::jobs::generate_route 2>&1 \
     | tee /tmp/test-wildside-backend-5-2-2-generate-route.out
   ```

7. Run `coderabbit review --agent` and resolve findings.

Commit when green.

### Milestone 3: define `EnrichmentJob`

Implement `EnrichmentJob` in `backend/src/domain/jobs/enrichment.rs`:

1. Introduce a `BoundingBox` newtype next to the job struct
   (or in `backend/src/domain/jobs/bounding_box.rs` if it grows beyond
   ~100 lines). The newtype wraps `[f64; 4]` in
   `[min_lng, min_lat, max_lng, max_lat]` order and validates at
   construction time:
   - `-180.0 <= min_lng < max_lng <= 180.0`,
   - `-90.0 <= min_lat < max_lat <= 90.0`,
   - all four components are finite.
   Provide a fallible constructor
   `BoundingBox::new(min_lng, min_lat, max_lng, max_lat)` returning
   `Result<Self, BoundingBoxError>`, together with `serde` derives that
   delegate to a `[f64; 4]` array representation so the wire format stays
   compatible with `OverpassEnrichmentRequest::bounding_box`.
2. Define the V1 envelope and payload using the signatures in
   "Interfaces and dependencies" below. Include `job_id: Uuid`,
   `idempotency_key: Option<IdempotencyKey>`, `bounding_box: BoundingBox`,
   `tags: Vec<String>`, and `enqueued_at: DateTime<Utc>`. Tags are
   represented as a sorted, deduplicated vector at construction time
   to keep canonical payloads stable, and the constructor rejects any
   tag list that exceeds `ENRICHMENT_JOB_V1_MAX_TAGS` or any individual
   tag that exceeds `ENRICHMENT_JOB_V1_MAX_TAG_LENGTH` bytes
   (`EnrichmentJobBuildError::TooManyTags` and
   `EnrichmentJobBuildError::TagTooLong` respectively). Place the
   schema-evolution doc-comment on the envelope so future agents see
   the rule before they edit V1.
3. Add `EnrichmentJob::to_overpass_request(&self) -> OverpassEnrichmentRequest`
   to give the existing Overpass worker a single conversion seam. Cover it
   with a unit test asserting the bounding-box ordering and tag list are
   preserved.
4. Add `rstest` unit tests for constructor validation, sort/dedupe of
   tags, serde round-trip, and `deny_unknown_fields`.
5. Add `proptest` strategies for bounding boxes and for whole jobs. Assert
   that:
   - Any value produced by the strategy round-trips through `serde_json`.
   - `EnrichmentJob::to_overpass_request` preserves the bounding box and
     the deduplicated tag set.
   - `BoundingBox::new` rejects inputs that violate the documented
     invariants.
6. Add an `insta` snapshot that locks the V1 JSON layout.
7. Run the focused suite:

   ```bash
   set -o pipefail
   cargo test -p backend domain::jobs::enrichment 2>&1 \
     | tee /tmp/test-wildside-backend-5-2-2-enrichment.out
   ```

8. Run `coderabbit review --agent` and resolve findings.

Commit when green.

### Milestone 4: behavioural and integration tests

Wire the new structs through the existing queue port without changing the
adapter contract.

1. Add an integration test file `backend/tests/job_structs_bdd.rs` and a
   feature file `backend/tests/features/job_structs.feature`. Follow the
   patterns in `docs/rstest-bdd-users-guide.md`.
2. Scenarios to cover:
   - "Build a generate-route job from a submission and enqueue via stub".
     Given a `RouteSubmissionRequest` with origin and destination, when the
     domain builds a `GenerateRouteJob` and enqueues it through a
     `StubRouteQueue<GenerateRouteJob>`, then the stub accepts it and
     records no error. Asserts the stub's logged outcome via the existing
     `tracing` test infrastructure.
   - "Reject an ill-formed submission".
     Given a `RouteSubmissionRequest` whose payload is not an object, when
     `GenerateRouteJob::try_from_submission` is called, then the builder
     returns the documented `GenerateRouteJobBuildError` variant.
   - "Build an enrichment job and observe its queue payload".
     Given an `EnrichmentJob::V1` with a known bounding box and tags, when
     the job is enqueued via a `FakeQueueProvider` wrapped in
     `GenericApalisRouteQueue<EnrichmentJob, _>`, then the recorded JSON
     payload matches the pinned snapshot fixture.
   - "Surface a serialization rejection".
     Given a deliberately panicking serde mock or a payload variant the
     codec cannot handle, when enqueue runs, then the adapter returns
     `JobDispatchError::Rejected` with the documented message shape. Use
     the existing `FailingQueueProvider` for the unavailable case.
   - "Convert to Overpass request".
     Given an `EnrichmentJob::V1`, when `to_overpass_request` is called,
     then the resulting `OverpassEnrichmentRequest` carries the same
     bounding box, tag list, and `job_id`.
3. Where a PostgreSQL-backed scenario is justified, reuse the embedded
   PostgreSQL harness described in
   `docs/pg-embed-setup-unpriv-users-guide.md` and follow the precedent in
   `backend/tests/route_queue_apalis_bdd.rs`. Keep these scenarios behind a
   tag so they remain skippable when embedded PostgreSQL is not
   available. If no scenario benefits from a live database, omit the
   PostgreSQL-backed path and record that in `Decision Log`.
4. Run the behavioural suite:

   ```bash
   set -o pipefail
   cargo test -p backend --test job_structs_bdd 2>&1 \
     | tee /tmp/test-wildside-backend-5-2-2-bdd.out
   ```

5. Run `coderabbit review --agent` and resolve findings.

Commit when green.

### Milestone 5: documentation, full gates, roadmap closure, and PR

1. Update documentation. Make only the edits the implementation justifies:
   - `docs/wildside-backend-architecture.md` — describe the V1 envelope,
     the `idempotency_key` carrying pattern, why trace propagation is
     deferred to 5.2.4, the conversion from `EnrichmentJob` to
     `OverpassEnrichmentRequest`, the antimeridian-wrap policy, and
     the worker-side rule that unknown envelope variants must be
     surfaced as `JobDispatchError::Rejected` (no panics, no silent
     drops; dead-letter routing lands with 5.2.3). Reference the new
     files by full path.
   - `docs/developers-guide.md` — add a "Background job payloads"
     section describing the envelope, the `try_from_submission` helper,
     snapshot review etiquette ("a snapshot diff implies a new
     variant, not a V1 edit"), the schema-evolution rule
     (`deny_unknown_fields` plus "additive changes require a new
     variant"), and the rule that domain code never imports Apalis.
   - `docs/users-guide.md` — record that user-facing behaviour has not
     changed and that idempotency, request IDs, and payload validation
     continue to work as before. No new endpoint or response shape is
     introduced.
   - `docs/backend-roadmap.md` — do not mark 5.2.2 done yet; it is marked
     only after the full gates pass.

2. Run documentation gates:

   ```bash
   set -o pipefail
   make markdownlint 2>&1 \
     | tee /tmp/markdownlint-wildside-backend-5-2-2.out

   set -o pipefail
   make nixie 2>&1 \
     | tee /tmp/nixie-wildside-backend-5-2-2.out
   ```

3. Run the full quality gates sequentially:

   ```bash
   set -o pipefail
   make check-fmt 2>&1 \
     | tee /tmp/check-fmt-wildside-backend-5-2-2.out

   set -o pipefail
   make lint 2>&1 \
     | tee /tmp/lint-wildside-backend-5-2-2.out

   set -o pipefail
   make test 2>&1 \
     | tee /tmp/test-wildside-backend-5-2-2.out
   ```

4. Run `coderabbit review --agent` after the full gates. Resolve every
   in-scope finding.

5. After every gate is green, mark only roadmap item 5.2.2 done in
   `docs/backend-roadmap.md`:

   ```markdown
   - [x] 5.2.2. Define job structs for `GenerateRouteJob` and `EnrichmentJob`.
   ```

   Commit the roadmap closure separately so the diff history is clean.

6. Run the closure gates again if the roadmap edit changes anything:

   ```bash
   set -o pipefail
   make check-fmt 2>&1 \
     | tee /tmp/check-fmt-wildside-backend-5-2-2-final.out

   set -o pipefail
   make lint 2>&1 \
     | tee /tmp/lint-wildside-backend-5-2-2-final.out

   set -o pipefail
   make test 2>&1 \
     | tee /tmp/test-wildside-backend-5-2-2-final.out
   ```

7. Run `coderabbit review --agent` one last time. Push the branch and
   update the draft PR (the plan PR opens first; see "Pull request"
   below).

## Interfaces and dependencies

Be prescriptive. At the end of milestone M3 the following symbols must
exist with these signatures.

In `backend/src/domain/jobs/mod.rs`:

```rust
//! Domain job payloads dispatched through `RouteQueue`.
//!
//! Each job type uses a `#[serde(tag = "v")]` envelope so new versions can
//! be added without breaking older consumers. Worker handlers match on the
//! envelope and dispatch to the right schema.

pub mod bounding_box;
pub mod enrichment;
pub mod generate_route;

pub use bounding_box::{BoundingBox, BoundingBoxError};
pub use enrichment::{EnrichmentJob, EnrichmentJobBuildError, EnrichmentJobV1};
pub use generate_route::{
    GenerateRouteJob, GenerateRouteJobBuildError, GenerateRouteJobV1,
};
```

In `backend/src/domain/jobs/generate_route.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{IdempotencyKey, UserId};
use crate::domain::ports::RouteSubmissionRequest;

/// Versioned envelope for route-generation jobs.
///
/// Adding a field to an existing variant requires cutting a new `V2`
/// variant. Do not relax `deny_unknown_fields` on an existing variant.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "v")]
pub enum GenerateRouteJob {
    #[serde(rename = "v1")]
    V1(GenerateRouteJobV1),
}

/// Version 1 payload for `GenerateRouteJob`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GenerateRouteJobV1 {
    /// Stable identifier for this submission, used for trace correlation.
    pub request_id: Uuid,
    /// Optional idempotency key supplied by the client.
    pub idempotency_key: Option<IdempotencyKey>,
    /// Authenticated user owning the request.
    pub user_id: UserId,
    /// Origin location identifier or coordinates, as supplied by the API.
    pub origin: serde_json::Value,
    /// Destination location identifier or coordinates.
    pub destination: serde_json::Value,
    /// Optional preference payload.
    #[serde(default)]
    pub preferences: Option<serde_json::Value>,
    /// Wall-clock time at which the job was built and enqueued. This is
    /// the payload-authoritative timestamp until 5.2.4 picks the
    /// `Parts::run_at` carriage; see Decision Log.
    pub enqueued_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenerateRouteJobBuildError {
    PayloadNotObject,
    PayloadMissingField { field: &'static str },
}
```

`PartialEq` is intentional and `Eq` is intentionally not derived because
the payload transitively contains `serde_json::Value`, which only
implements `PartialEq`. Implement `Display` and `std::error::Error` for
the error using the existing macro patterns; the snippet above shows the
variants only.

In `backend/src/domain/jobs/bounding_box.rs`:

```rust
use serde::{Deserialize, Serialize};

/// WGS84 bounding box in `[min_lng, min_lat, max_lng, max_lat]` order.
///
/// Antimeridian-wrapped boxes are not supported in V1 (`min_lng` must be
/// strictly less than `max_lng`). Callers spanning the dateline must
/// split the box into two pieces client-side.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "[f64; 4]", into = "[f64; 4]")]
pub struct BoundingBox {
    coords: [f64; 4],
}

impl BoundingBox {
    pub fn new(
        min_lng: f64,
        min_lat: f64,
        max_lng: f64,
        max_lat: f64,
    ) -> Result<Self, BoundingBoxError> { /* validate then store */ }

    pub fn coords(&self) -> [f64; 4] { self.coords }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BoundingBoxError {
    NonFinite,
    LongitudeOutOfRange,
    LatitudeOutOfRange,
    InvertedOrdering,
    AntimeridianWrap,
}
```

The newtype derives only `PartialEq`. `f64` is not `Eq`, so neither
`BoundingBox` nor any struct embedding it can implement `Eq`. The whole
envelope tree is `PartialEq`-only as a result; this is the deliberate
choice recorded in `Decision Log`.

In `backend/src/domain/jobs/enrichment.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::IdempotencyKey;
use crate::domain::jobs::BoundingBox;
use crate::domain::ports::OverpassEnrichmentRequest;

/// Maximum number of tags carried on a V1 enrichment job.
pub const ENRICHMENT_JOB_V1_MAX_TAGS: usize = 64;
/// Maximum UTF-8 length (bytes) of any single tag in V1.
pub const ENRICHMENT_JOB_V1_MAX_TAG_LENGTH: usize = 64;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "v")]
pub enum EnrichmentJob {
    #[serde(rename = "v1")]
    V1(EnrichmentJobV1),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnrichmentJobV1 {
    pub job_id: Uuid,
    pub idempotency_key: Option<IdempotencyKey>,
    pub bounding_box: BoundingBox,
    /// Sorted, deduplicated tag list. Bounded by
    /// `ENRICHMENT_JOB_V1_MAX_TAGS` and per-tag
    /// `ENRICHMENT_JOB_V1_MAX_TAG_LENGTH` at construction time.
    pub tags: Vec<String>,
    pub enqueued_at: DateTime<Utc>,
}

impl EnrichmentJob {
    /// Convert any envelope variant into the existing Overpass port
    /// request shape. V1 is infallible; future variants whose conversion
    /// can fail must return `Result<OverpassEnrichmentRequest,
    /// EnrichmentJobConversionError>` instead and a Decision Log entry
    /// must capture the change.
    pub fn to_overpass_request(&self) -> OverpassEnrichmentRequest { /* ... */ }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnrichmentJobBuildError {
    BoundingBox(BoundingBoxError),
    EmptyTags,
    TooManyTags { limit: usize, observed: usize },
    TagTooLong { limit: usize, observed: usize },
}
```

`to_overpass_request` lives on the envelope, not on `V1`, so workers
always go through the version-aware seam.

In `backend/Cargo.toml`'s `[dev-dependencies]` (new entries only):

```toml
pretty_assertions = "1"
googletest = "0.13"
```

If neither crate compiles cleanly on edition 2024, stop and escalate.

## Validation and acceptance

5.2.2 may be marked done only when every clause below is true.

Functional acceptance:

- `backend::domain::jobs::GenerateRouteJob` and
  `backend::domain::jobs::EnrichmentJob` exist with the documented
  envelope and V1 payload shapes.
- `GenerateRouteJob::try_from_submission` accepts well-formed
  `RouteSubmissionRequest` payloads and returns the documented
  `GenerateRouteJobBuildError` variants on ill-formed input.
- `EnrichmentJob::to_overpass_request` returns a value-equal
  `OverpassEnrichmentRequest`.
- `BoundingBox::new` rejects non-finite inputs, longitudes outside
  `[-180.0, 180.0]`, latitudes outside `[-90.0, 90.0]`, inverted
  ordering, and antimeridian-wrapped boxes (`min_lng >= max_lng`).
- `EnrichmentJobV1` construction rejects tag vectors that exceed
  `ENRICHMENT_JOB_V1_MAX_TAGS` and individual tags whose UTF-8 byte
  length exceeds `ENRICHMENT_JOB_V1_MAX_TAG_LENGTH`, with the named
  error variants documented above.
- The V1 JSON shape is locked under at least one `insta` snapshot per job
  type. Updating a snapshot requires explicit human review.
- The new structs can be enqueued through `StubRouteQueue<P>` and through
  `GenericApalisRouteQueue<P, FakeQueueProvider>` without changing
  `RouteQueue`'s signature.

Test acceptance:

- `cargo test -p backend domain::jobs` passes.
- `cargo test -p backend --test job_structs_bdd` passes.
- `proptest` strategies exhaust at least 256 cases per property without
  finding a counter-example (the default is 256; do not lower it).
- `insta` snapshots are committed and `cargo insta test --check` succeeds
  during `make test`.

Gate acceptance:

- `make check-fmt`, `make lint`, `make markdownlint`, `make nixie`, and
  `make test` all pass.
- `coderabbit review --agent` reports no unresolved in-scope concerns
  after each major milestone.
- The branch tracks
  `origin/backend-5-2-2-job-structs-for-generate-route-and-enrichment`
  and the draft PR carries the roadmap item number in the title.

Property and proof scope:

- `proptest` covers serde round-trips and bounding-box invariants.
- `kani` and `verus` are not required. There is no unbounded invariant
  beyond serde round-trip and bounding-box validation, and both are
  already well covered by property tests. Re-evaluate if a future
  milestone introduces invariants on retry sequencing or schema
  migration.

## Idempotence and recovery

All commands above are safe to rerun. `insta` snapshot review is
human-gated; rerunning the test suite will not silently overwrite a
snapshot. If a commit interleaves with a snapshot regeneration, run
`cargo insta pending-snapshots` to reconcile before the next commit.

If the focused queue suite or the BDD suite fails because embedded
PostgreSQL is unavailable, record the log path under
`Surprises & Discoveries` and stop; do not rewrite the test harness.

No destructive Git command is required. Do not run `git reset --hard` or
`git checkout --` to discard work unless the user explicitly asks for it.

## Pull request

The plan ships in two PRs:

1. The plan PR. Open immediately after drafting this file, in draft
   state, on the branch
   `backend-5-2-2-job-structs-for-generate-route-and-enrichment`. The
   title must include `(5.2.2)`. The body must reference this ExecPlan by
   path and include a `## References` section linking to the Lody session
   recorded by `${LODY_SESSION_ID}`.
2. The implementation PR. Once approval is granted and the milestones
   complete, update or replace the plan PR with the implementation diff.
   The body must include gate logs, CodeRabbit outcomes, and any updates
   to the documentation sections listed in milestone M5.

## Progress

- [x] (2026-06-06 01:30Z) Loaded `leta`, `rust-router`,
  `hexagonal-architecture`, `execplans`, `firecrawl-mcp`, `pr-creation`,
  and supporting Rust skills for planning.
- [x] (2026-06-06 01:31Z) Added the worktree as a Leta workspace.
- [x] (2026-06-06 01:31Z) Renamed the local branch to
  `backend-5-2-2-job-structs-for-generate-route-and-enrichment`.
- [x] (2026-06-06 01:35Z) Surveyed the roadmap, the wildside backend
  architecture, the queue port, the route submission port, the Overpass
  enrichment port, the existing Apalis adapter, and the 5.2.1 ExecPlan to
  set the baseline.
- [x] (2026-06-06 01:40Z) Used a research agent team to gather Apalis 1.0
  idioms (no `Job` trait in 1.0; metadata lives in `Parts`;
  `PostgresStorage<Args>` accepts any `Serialize + DeserializeOwned`
  type; rc.6/rc.7 lack the rc.8 framework-native idempotency feature) and
  to confirm the concrete fields the new structs must carry.
- [x] (2026-06-06 01:50Z) Drafted this ExecPlan.
- [x] (2026-06-14 23:37Z) Approval received from the user; implementation
  started under this ExecPlan.
- [ ] Logisphere expert review run before delivery (planning gate).
- [x] (2026-06-14 23:37Z) Milestone 0 baseline audit confirmed the branch
  name, empty working tree, and absence of existing `GenerateRouteJob` /
  `EnrichmentJob` symbols.
- [x] (2026-06-14 23:58Z) Milestone 1 scaffold compiled with
  `cargo check -p backend` and CodeRabbit retry completed with
  `findings: 0`.
- [x] (2026-06-14 23:59Z) Milestone 1 scaffold committed as `8219cf9`.
- [x] (2026-06-15 00:30Z) Milestone 2 `GenerateRouteJob` passed red/green
  focused tests, `make check-fmt`, `make lint`, and CodeRabbit with
  `findings: 0`.
- [x] (2026-06-15 00:32Z) Milestone 2 `GenerateRouteJob` committed as
  `39e935d`.
- [x] (2026-06-15 01:12Z) Milestone 3 `EnrichmentJob` passed red/green
  focused tests, `make check-fmt`, `make lint`, and CodeRabbit with
  `findings: 0`.
- [x] (2026-06-15 01:14Z) Milestone 3 `EnrichmentJob` committed as
  `f1d354b`.
- [x] (2026-06-15 01:47Z) Milestone 4 behavioural tests passed
  `cargo test -p backend --test job_structs_bdd`, `make check-fmt`,
  `make lint`, and CodeRabbit with `findings: 0`.
- [x] (2026-06-15 01:48Z) Milestone 4 behavioural tests committed as
  `7f02a3c`.
- [x] (2026-06-15 03:32Z) Milestone 5 documentation gates passed:
  `make fmt`, `make markdownlint`, and `make nixie`.
- [x] (2026-06-15 03:32Z) Milestone 5 full quality gates passed:
  `make check-fmt`, `make lint`, and `make test`. The full test gate ran 1316
  Rust nextest cases with 1316 passed and 4 skipped, then passed the
  TypeScript/Vitest workspace tests.
- [x] (2026-06-15 03:45Z) Milestone 5 documentation CodeRabbit review
  completed with `findings: 0`.
- [ ] Milestone 5 documentation, full gates, roadmap closure, and PR
  update complete.

## Surprises & discoveries

- (2026-06-06 01:38Z) `googletest` and `pretty_assertions` are absent
  from the workspace `Cargo.toml` files
  (`backend/Cargo.toml`, `backend/crates/pagination/Cargo.toml`, and the
  workspace root). The task description requires `googletest` assertions
  and `pretty_assertions` for clear test semantics, so milestone M1 must
  add them as dev-dependencies. If the approver prefers to skip these
  additions, downgrade test rigour by falling back to `assert_eq!` plus
  explicit failure context strings.
- (2026-06-06 01:40Z) `apalis_core` 1.0 has no `Job` trait; the wildside
  adapter is already correct in treating the queue's `Plan` as a serde
  type. This is captured in the architecture doc update for milestone M5.
- (2026-06-06 01:41Z) The current Apalis storage is
  `PostgresStorage<serde_json::Value>`. Switching to typed
  `PostgresStorage<GenerateRouteJob>` or to `SharedPostgresStorage` is
  attractive but is queue-adapter territory and belongs with 5.3.1. The
  plan explicitly leaves the storage shape unchanged.
- (2026-06-06 01:42Z) `backend::domain::trace_id::TraceId` is not
  serde-derived. The plan does not modify it. Trace propagation is
  roadmap item 5.2.4 and will revisit this in its own approved scope.

- (2026-06-06 01:55Z) Logisphere design review of the DRAFT plan
  surfaced a compile-time inconsistency (`Eq` on the envelope vs only
  `PartialEq` on `BoundingBox`) and three operational gaps
  (unbounded tags, no published schema-evolution rule, no documented
  worker-side policy for unknown envelope variants). All findings have
  been folded back into Risks, Decision Log, and the milestone steps.

- (2026-06-14 23:37Z) The current branch is already named
  `backend-5-2-2-job-structs-for-generate-route-and-enrichment`, matching the
  plan. `git status --short` reported an empty working tree before
  implementation edits began.

- (2026-06-14 23:46Z) The first milestone 1 CodeRabbit invocation reached
  sandbox preparation and then produced no output for about five minutes. Only
  the `coderabbit review --agent` process tied to
  `/tmp/coderabbit-wildside-backend-5-2-2-scaffold.out` was terminated; other
  agents' CodeRabbit processes were left untouched. A retry was started under
  `/tmp/coderabbit-wildside-backend-5-2-2-scaffold-retry.out`.

- (2026-06-15 00:18Z) `make lint` found two deterministic issues before the
  milestone 2 CodeRabbit review: clippy rejected the approved
  seven-argument `GenerateRouteJob::v1` signature, and Whitaker did not treat
  helper fixtures in `backend/src/domain/jobs/generate_route/tests.rs` as
  test functions for `.expect()` usage. The constructor now has a scoped
  `#[expect(clippy::too_many_arguments)]` with a reason, and fixture helpers
  use deterministic UUID constructors or explicit `match` panics.

- (2026-06-15 01:02Z) `make lint` found two deterministic enrichment issues
  before CodeRabbit: clippy rejected the approved five-argument
  `EnrichmentJob::v1` constructor, and the parameterized invalid-bounding-box
  test expanded to a helper with too many arguments. The constructor now has a
  scoped `#[expect(clippy::too_many_arguments)]`; the test now passes one
  structured case value per row.

- (2026-06-15 01:40Z) Milestone 4 did not add a PostgreSQL-backed scenario.
  `GenericApalisRouteQueue<EnrichmentJob, FakeQueueProvider>` exercises the
  typed plan serialization and queue-provider seam without changing the
  `PostgresStorage<serde_json::Value>` storage shape reserved for 5.3.1.

- (2026-06-15 02:05Z) Milestone 5 documentation gates exposed a pre-existing
  Mermaid parse failure in
  `docs/rstest-bdd-v0-5-0-migration-guide.md`. `merman-cli` reported the
  migration flowchart as an unterminated node label; simplifying the diagram to
  parser-safe quoted labels, without changing the surrounding prose, unblocks
  `make nixie`.

## Decision log

- Decision: Define V1 job structs under a `#[serde(tag = "v")]` envelope
  rather than a flat `schema_version: u32` field.
  Rationale: tagged enums let `serde` parse old and new variants with no
  branchy decode logic, and worker handlers can `match` on the envelope
  cleanly. The envelope adds two tokens to the wire format; the
  forward-compatibility benefit dominates.
  Date/Author: 2026-06-06 / planning agent.

- Decision: Carry an explicit `idempotency_key: Option<IdempotencyKey>`
  on each V1 payload.
  Rationale: `apalis-core` 1.0.0-rc.7 (the current pin) lacks the
  framework-native idempotency feature added in rc.8. Carrying the key
  ourselves keeps the same shape working before and after an Apalis
  upgrade; the field can later be mapped onto
  `TaskBuilder::id(idempotency_key.into())` without breaking the wire
  shape.
  Date/Author: 2026-06-06 / planning agent.

- Decision: Defer trace-identifier fields to roadmap 5.2.4.
  Rationale: roadmap 5.2.4 explicitly owns trace propagation through job
  metadata. Adding a `trace_id` field now would require revising
  `TraceId` to derive serde and would commit the job's wire shape to a
  carrier that may not be chosen later (the Apalis OpenTelemetry layer
  is the likely path). The `v` envelope leaves room for a V2 schema that
  adds trace metadata after 5.2.4 selects the carrier.
  Date/Author: 2026-06-06 / planning agent.

- Decision: Wrap the bounding box in a validating newtype rather than
  exposing `[f64; 4]` directly.
  Rationale: the current Overpass enrichment port accepts
  `bounding_box: [f64; 4]` without validation
  (`backend/src/domain/ports/overpass_enrichment_source.rs:14`). The job
  payload is durable persisted state; persisting nonsensical
  coordinates would be much harder to recover from than rejecting them
  at construction. The newtype validates once and delegates to
  `[f64; 4]` on the wire so the Overpass port stays compatible.
  Date/Author: 2026-06-06 / planning agent.

- Decision: Keep `RouteQueue::Plan` generic and do not collapse it to a
  concrete enum like `Job::{GenerateRoute(...), Enrichment(...)}`.
  Rationale: keeping `Plan` generic means each adapter instance carries
  exactly one job type. The architecture doc already plans for two
  queues, one per job type. A monolithic enum would block this and would
  force every adapter to know about every job, violating the hexagonal
  boundary.
  Date/Author: 2026-06-06 / planning agent.

- Decision: Do not introduce a `From<RouteSubmissionRequest>` impl.
  Rationale: building a `GenerateRouteJob` from a submission is fallible
  (missing origin or destination, non-object payload). `From` would
  invite silent panics. `try_from_submission` (named, fallible, and
  taking the extra `request_id`/`enqueued_at` parameters) keeps the
  intent explicit.
  Date/Author: 2026-06-06 / planning agent.

- Decision: Keep the approved `GenerateRouteJob::v1` positional constructor
  and use a scoped clippy expectation for `too_many_arguments`.
  Rationale: the ExecPlan explicitly prescribes the constructor signature so
  tests and later milestones can build V1 payloads without introducing an
  additional builder type. Clippy correctly flags the risk, so the
  expectation is limited to that function and documents that the argument list
  mirrors the persisted schema fields.
  Date/Author: 2026-06-15 / implementation agent.

- Decision: Keep the approved `EnrichmentJob::v1` positional constructor and
  use a scoped clippy expectation for `too_many_arguments`.
  Rationale: the constructor mirrors the V1 durable payload fields and avoids
  introducing an extra builder solely to satisfy a lint. The expectation is
  limited to the constructor and carries the same schema-shape rationale as
  `GenerateRouteJob::v1`.
  Date/Author: 2026-06-15 / implementation agent.

- Decision: Derive only `PartialEq` (not `Eq`) on the job envelopes and
  on `BoundingBox`.
  Rationale: `BoundingBox` wraps `[f64; 4]`, and `GenerateRouteJobV1`
  embeds `serde_json::Value` for `origin`, `destination`, and
  `preferences`. Neither `f64` nor `serde_json::Value` implements `Eq`.
  An "i32 microdegrees" workaround for `BoundingBox` would still leave
  the route job's `serde_json::Value` fields without `Eq`, so the
  whole envelope tree drops `Eq` for consistency. Test code that wants
  hashing-based comparison should use snapshot equality or
  `pretty_assertions::assert_eq!` on the serialised form.
  Date/Author: 2026-06-06 / planning agent (post-Logisphere review).

- Decision: Use human-readable string tags (`"v1"`) for the envelope
  discriminator rather than numeric (`"1"`).
  Rationale: ops staff inspecting `apalis.jobs` rows in `psql` will see
  `"v": "v1"` and immediately understand the discriminator. The wire
  cost is one extra byte per job, which is negligible.
  Date/Author: 2026-06-06 / planning agent (post-Logisphere review).

- Decision: V1 does not support antimeridian-wrapped bounding boxes.
  Rationale: representing a wrap requires either a tagged geometry type
  or a sentinel that fights `min_lng < max_lng` validation. Both make
  the V1 contract less obvious. Wildside's launch geofences do not
  cross the dateline; callers in that situation must split the box
  client-side. The named error variant `AntimeridianWrap` makes the
  rejection self-describing, and a future V2 can lift the restriction
  cleanly under its own snapshot.
  Date/Author: 2026-06-06 / planning agent (post-Logisphere review).

- Decision: Bound `EnrichmentJobV1::tags` at construction time.
  Rationale: `apalis.jobs` rows are persisted JSON; an unbounded tag
  vector turns a bug into a queue-table footprint problem. The bounds
  (`64` tags, `64` UTF-8 bytes each) are generous compared to known
  Overpass tag-set sizes (single-digit count is typical) and the error
  surface is one extra variant.
  Date/Author: 2026-06-06 / planning agent (post-Logisphere review).

- Decision: Place `to_overpass_request` on the `EnrichmentJob` envelope,
  not on `EnrichmentJobV1`.
  Rationale: workers should not match the envelope a second time to
  reach the conversion. Putting the method on the envelope keeps every
  call site version-agnostic, and the doc-comment commits the V1
  conversion to be infallible while documenting that any future variant
  whose conversion can fail must change the return type and earn a
  Decision Log entry.
  Date/Author: 2026-06-06 / planning agent (post-Logisphere review).

- Decision: Schema-evolution rule is published with the V1 types.
  Rationale: `deny_unknown_fields` is safe but the safety only holds if
  future agents know that adding a field requires a new variant. A
  one-line doc-comment on each envelope (and a matching paragraph in
  the developers guide) makes the rule load-bearing rather than
  accidental.
  Date/Author: 2026-06-06 / planning agent (post-Logisphere review).

- Open question deferred to 5.2.4: whether `enqueued_at` on the payload
  remains authoritative or whether the queue's `Parts::run_at` becomes
  the source of truth for SLO accounting. V1 carries `enqueued_at` for
  now so the field's absence is not the blocker; the trade-off is
  noted so 5.2.4 can decide once trace metadata carriage is settled.

- Open question deferred to a future plan: whether `idempotency_key`
  should be tightened from `Option` to required. Today it mirrors the
  `RouteSubmissionRequest::idempotency_key` shape, where the HTTP layer
  treats it as optional. When the workspace adopts Apalis rc.8+
  framework-native idempotency, the payload field must be removed in
  the same PR that wires `TaskBuilder::id` to avoid the duplicate
  source-of-truth failure mode recorded in Risks.

- Decision: Keep PostgreSQL-backed behavioural scenarios optional in
  milestone M4.
  Rationale: 5.2.2 introduces serializable types only. Most behaviour is
  observable through `StubRouteQueue` and `FakeQueueProvider` already.
  Spending embedded PostgreSQL minutes here only pays off when 5.2.3 and
  5.3.1 add worker consumption. The decision is revisited only if a
  serialization quirk surfaces during the Apalis adapter integration
  smoke test.
  Date/Author: 2026-06-06 / planning agent.

## Outcomes & retrospective

To be completed at the end of milestone M5. Compare the result against
the purpose at the top of this plan, note what was discovered (especially
about future trace propagation and storage-shape decisions for 5.2.3 and
5.3.1), and capture any tooling or test-pattern improvements that would
help future job-struct work (for example, whether the envelope pattern
should be lifted to a generic `VersionedPayload<T>` helper crate).

## Revision history

- 2026-06-06: Initial DRAFT.
- 2026-06-06: Folded findings from the Logisphere design review into
  `Risks`, `Decision Log`, `Surprises & Discoveries`, the interface
  signatures, the M3 step list, and the M5 documentation step. Notable
  changes: dropped `Eq` from envelopes and `BoundingBox` (an `f64` and
  `serde_json::Value` reality check); bounded `EnrichmentJobV1::tags`;
  switched the envelope discriminator to `"v1"`; moved
  `to_overpass_request` to the envelope; documented the antimeridian
  policy and the schema-evolution rule; captured the
  `enqueued_at`/`Parts::run_at` and `idempotency_key` open questions for
  5.2.4 and the future Apalis upgrade. No tolerances or scope budgets
  shifted; the implementation surface remains the same files and
  modules, only the type derives and a small number of validation
  constants changed.
