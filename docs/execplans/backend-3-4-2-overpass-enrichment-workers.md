# Deliver Overpass enrichment workers with quotas, circuit breaking, and metrics (roadmap 3.4.2)

This Execution Plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

This plan covers roadmap item 3.4.2 only:
`Add Overpass enrichment workers with semaphore-governed quotas, circuit
breaking, and metrics wired to the enrichment job counters`.

## Purpose / big picture

Roadmap item 3.4.2 adds asynchronous Overpass enrichment workers so sparse POI
coverage can be improved without blocking request-handling paths. The worker
flow must honour strict quotas, avoid cascading failures with circuit breaking,
and emit metrics suitable for production dashboards and alerts.

After this work, the backend should be able to enqueue and execute enrichment
jobs through domain ports, call Overpass through an outbound adapter under
semaphore and quota controls, upsert returned POIs through existing persistence
ports, and expose accurate enrichment job counters.

Observable success criteria:

- Enrichment jobs run in worker mode and do not execute Overpass calls inline
  in HTTP handlers.
- Quotas are enforced with deterministic behaviour for both allowed and denied
  calls.
- Outbound Overpass calls are gated by a semaphore with a documented default
  concurrency.
- Circuit breaker state transitions are exercised in tests and block outbound
  calls when open.
- Enrichment metrics are wired to existing job counters and include explicit
  success and failure outcomes.
- Unit tests (`rstest`) and behavioural tests (`rstest-bdd`) cover happy,
  unhappy, and edge paths.
- Behavioural tests run with embedded PostgreSQL support via
  `pg-embedded-setup-unpriv`.
- `docs/wildside-backend-architecture.md` records the 3.4.2 design decisions.
- `docs/backend-roadmap.md` marks 3.4.2 done only after required gates pass.
- `make check-fmt`, `make lint`, and `make test` pass with retained logs.

## Constraints

- Scope is roadmap item 3.4.2 only. Do not implement 3.4.3 admin reporting or
  enrichment provenance exposure in this change.
- Preserve hexagonal architecture boundaries:
  - domain defines ports, orchestration rules, and domain error types;
  - outbound adapters implement persistence, queue, metrics, and Overpass I/O;
  - inbound and worker adapters orchestrate through ports only.
- Keep dependency direction intact:
  - `domain` must not import `inbound` or `outbound`;
  - `inbound` must not import `outbound` implementation details directly.
- Keep POI persistence through existing domain repository ports and outbound
  Diesel adapters; do not add direct SQL calls in worker orchestration modules.
- Use semaphore-governed Overpass calls with a documented default permit count.
- Implement circuit breaker semantics in domain logic (or domain-owned policy
  module) with adapter-neutral state transitions and error mapping.
- Wire metrics through domain metrics ports and outbound Prometheus adapters.
- Use `rstest` for unit coverage and `rstest-bdd` for behaviour coverage.
- Use existing embedded PostgreSQL support patterns under `backend/tests/support`
  and `pg-embedded-setup-unpriv` guidance.
- Keep Markdown documentation in en-GB-oxendict and 80-column prose wrapping.

## Tolerances (exception triggers)

- Scope tolerance: if 3.4.3 work becomes necessary to satisfy 3.4.2, stop and
  split work into a follow-up plan.
- Storage tolerance: if global quota correctness requires schema changes beyond
  one cohesive migration set, stop and split into a staged migration plan.
- Churn tolerance: if implementation exceeds 30 files or 2,200 net LOC, stop
  and re-scope into two sequenced execution milestones.
- Behaviour tolerance: if circuit breaker policy cannot be expressed without
  leaking adapter details into domain modules, stop and redesign port surfaces.
- Gate tolerance: if any required gate (`check-fmt`, `lint`, `test`) fails more
  than three consecutive fix attempts, stop with retained logs and root-cause
  notes.
- Runtime tolerance: if embedded PostgreSQL tests are unstable under default
  parallelism, run with constrained test threads, document rationale, and keep
  full coverage enabled.

## Risks

- Risk: quota state may diverge across worker replicas if implemented process
  local only.
  Mitigation: define quota storage scope explicitly (shared backend state) and
  test concurrent worker behaviour against that scope.

- Risk: circuit breaker thresholds may over-trigger and suppress useful
  enrichment.
  Mitigation: define threshold, cooldown, and half-open probe semantics in one
  policy module with deterministic unit tests.

- Risk: semaphore control may accidentally wrap whole jobs instead of just
  outbound calls, reducing throughput.
  Mitigation: apply permit acquisition only at Overpass call boundaries and
  assert release behaviour in tests.

- Risk: metrics wiring may drift from the counters used in dashboards.
  Mitigation: centralize metric names and label enums in one port/adapter pair
  and assert emission in both unit and behavioural tests.

- Risk: behavioural tests may fail intermittently due to cluster setup.
  Mitigation: reuse `backend/tests/support` shared-cluster helpers and explicit
  skip handling already used by ingestion BDD suites.

## Agent team

Use a four-agent ownership model for design and implementation. Ownership is
strict so each agent edits only its assigned files unless a handoff is
explicitly recorded in `Decision Log`.

- Agent A: domain contracts and orchestration.
  Owns:
  - domain ports for enrichment source, quota/circuit policy inputs, and
    enrichment metrics;
  - enrichment worker orchestration service and domain error mapping;
  - unit tests for policy state transitions and orchestration outcomes.

- Agent B: outbound adapters and persistence integration.
  Owns:
  - Overpass HTTP adapter and retry/jitter plumbing;
  - queue adapter wiring and any required adapter configuration;
  - metrics adapter implementation and POI upsert integration touchpoints.

- Agent C: worker runtime and composition wiring.
  Owns:
  - worker-mode job registration and execution path for enrichment jobs;
  - app/state builder wiring so worker code receives domain ports, not concrete
    outbound types;
  - configuration surface for quota, timeout, semaphore, and breaker settings.

- Agent D: behavioural tests and documentation.
  Owns:
  - `rstest-bdd` feature file and step bindings for enrichment scenarios;
  - `pg-embedded-setup-unpriv` fixture integration and skip behaviour;
  - architecture decision updates and roadmap checkbox closure on completion.

Coordination rules:

1. Design sequence: A -> B -> C -> D.
2. Integration sequence: merge A+B first, then C, then D.
3. Each merge point runs targeted tests before proceeding.
4. Final integrated run executes all required gates with retained logs.

## Context and orientation

Primary documentation references:

- `docs/backend-roadmap.md` section 3.4.2 for required deliverable wording.
- `docs/wildside-backend-architecture.md` sections describing:
  - ports-and-adapters invariants;
  - ingestion/enrichment worker behaviour and Overpass limits;
  - observability counters for enrichment jobs.
- `docs/rust-testing-with-rstest-fixtures.md` for fixture and parameterization
  patterns.
- `docs/rstest-bdd-users-guide.md` for scenario composition and fixture-driven
  world management.
- `docs/pg-embed-setup-unpriv-users-guide.md` for embedded PostgreSQL bootstrap
  and shared-cluster use.
- `docs/rust-doctest-dry-guide.md` and
  `docs/complexity-antipatterns-and-refactoring-strategies.md` for maintainable
  test and decomposition discipline.

Current code anchors to inspect before editing:

- `backend/src/domain/ports/route_queue.rs`
- `backend/src/outbound/queue/mod.rs`
- `backend/src/domain/route_submission/mod.rs`
- `backend/src/domain/ports/idempotency_metrics.rs`
- `backend/src/outbound/metrics/mod.rs`
- `backend/src/outbound/metrics/prometheus_idempotency.rs`
- `backend/src/domain/ports/osm_poi_repository.rs`
- `backend/src/outbound/persistence/diesel_osm_poi_repository.rs`
- `backend/tests/osm_ingestion_bdd.rs`
- `backend/tests/osm_ingestion_bdd/world.rs`
- `backend/tests/support/embedded_postgres.rs`
- `backend/tests/support/cluster_skip.rs`

## Milestones

## Milestone 0 - Baseline and seam confirmation

Confirm current job-queue, worker, and metrics seams before introducing new
ports. Record exactly where enrichment jobs will be enqueued and where they are
consumed.

Deliverables:

- Short baseline notes in `Surprises & Discoveries` confirming no boundary
  violations in planned touchpoints.
- Initial `Decision Log` entries for chosen quota and breaker state scope.

Validation:

```bash
set -o pipefail
make test | tee /tmp/test-$(get-project)-$(git branch --show)-baseline.out
```

Expected evidence:

```plaintext
Existing suite passes or known unrelated failures are documented before edits.
```

## Milestone 1 - Domain ports and policy model

Add or extend domain ports for enrichment source interactions, metrics
recording, and any quota/circuit policy dependencies. Implement policy modules
for:

- quota accounting and allow/deny outcomes;
- semaphore-aware call admission;
- circuit breaker state transitions and cooldown logic.

Keep implementations adapter-agnostic and backed by `rstest` unit tests.

Deliverables:

- New/updated domain ports in `backend/src/domain/ports/*`.
- Enrichment domain orchestration module under `backend/src/domain/*`.
- Unit tests covering happy and unhappy policy paths.

Validation:

```bash
set -o pipefail
make test | tee /tmp/test-$(get-project)-$(git branch --show)-milestone1.out
```

Expected evidence:

```plaintext
New unit tests fail before implementation and pass after implementation.
```

## Milestone 2 - Outbound adapters and worker wiring

Implement outbound adapters for Overpass calls and enrichment metrics emission,
then wire enrichment worker execution so jobs use domain ports and run under
configured semaphore, quota, and circuit rules.

Deliverables:

- Overpass outbound adapter module.
- Metrics adapter updates for enrichment counters.
- Worker runtime/composition updates to register enrichment job handling.
- Adapter-level tests for HTTP error mapping, retry behaviour, and timeout
  handling.

Validation:

```bash
set -o pipefail
make test | tee /tmp/test-$(get-project)-$(git branch --show)-milestone2.out
```

Expected evidence:

```plaintext
Adapter contract tests pass and worker execution paths remain port-driven.
```

## Milestone 3 - Behavioural tests with embedded PostgreSQL

Add `rstest-bdd` scenarios for end-to-end worker behaviour using the embedded
PostgreSQL support path. Reuse existing support fixtures where possible.

Required scenarios:

- successful enrichment updates POIs and increments success counters;
- quota exhausted prevents outbound calls and increments quota-denied counters;
- repeated failures open the circuit and later jobs short-circuit;
- half-open probe success closes the circuit;
- timeout and retry exhaustion increments failure counters;
- semaphore limits concurrent outbound Overpass calls.

Deliverables:

- `backend/tests/features/overpass_enrichment.feature`
- `backend/tests/overpass_enrichment_bdd.rs`
- `backend/tests/overpass_enrichment_bdd/world.rs`

Validation:

```bash
set -o pipefail
make test | tee /tmp/test-$(get-project)-$(git branch --show)-milestone3.out
```

Expected evidence:

```plaintext
BDD scenarios pass with pg-embedded setup and include unhappy-path assertions.
```

## Milestone 4 - Documentation, roadmap closure, and full gates

Update architecture decisions and mark roadmap item 3.4.2 complete only after
all required gates pass.

Deliverables:

- Architecture decision entry in `docs/wildside-backend-architecture.md`
  describing:
  - driving and driven ports;
  - quota scope and semaphore default;
  - circuit breaker semantics;
  - metrics names/labels and alert intent.
- Roadmap update in `docs/backend-roadmap.md`:
  - change `- [ ] 3.4.2 ...` to `- [x] 3.4.2 ...` only after final gates pass.

Final validation commands:

```bash
set -o pipefail
make check-fmt | tee /tmp/check-fmt-$(get-project)-$(git branch --show).out
make lint | tee /tmp/lint-$(get-project)-$(git branch --show).out
make test | tee /tmp/test-$(get-project)-$(git branch --show).out
```

Expected evidence:

```plaintext
All three commands exit 0; retained logs show success without unresolved gates.
```

## Progress

- [x] (2026-02-26) Confirmed branch context:
      `backend-3-4-2-overpass-enrichment-workers`.
- [x] (2026-02-26) Loaded `execplans`, `leta`, and `hexagonal-architecture`
      skill guidance for plan construction.
- [x] (2026-02-26) Reviewed roadmap 3.4.2 scope and architecture/testing
      references.
- [x] (2026-02-26) Used a focused agent team to gather:
      architecture constraints, testing matrix, and documentation guardrails.
- [x] (2026-02-26) Authored this ExecPlan at
      `docs/execplans/backend-3-4-2-overpass-enrichment-workers.md`.
- [x] (2026-02-26) Implemented Milestone 0 baseline confirmation using domain,
      outbound, and BDD seam inspection before edits.
- [x] (2026-02-26) Implemented Milestone 1 domain ports and worker policy
      modules with `rstest` unit coverage.
- [x] (2026-02-26) Implemented Milestone 2 outbound Overpass and Prometheus
      adapters with targeted unit tests.
- [x] (2026-02-26) Implemented Milestone 3 behavioural coverage via
      `rstest-bdd` and embedded PostgreSQL.
- [x] (2026-02-26) Implemented Milestone 4 documentation closure and final
      gates (`make check-fmt`, `make lint`, `make test`).

## Surprises & Discoveries

- Observation (2026-02-26): architecture guidance already defines explicit
  Overpass limits and enrichment observability targets, which reduces design
  ambiguity for 3.4.2.
  Impact: this plan can pin concrete quota, timeout, semaphore, and metrics
  expectations early.

- Observation (2026-02-26): existing ingestion BDD support includes reusable
  embedded PostgreSQL and skip-handling helpers.
  Impact: 3.4.2 behaviour tests can reuse proven fixtures rather than creating
  a second cluster bootstrap path.

- Observation (2026-02-26): `cargo test <filter>` can compile BDD crates
  without executing the scenario functions when the filter misses generated
  names.
  Impact: behavioural evidence should use explicit `--test` invocations for new
  BDD files.

## Decision Log

- Decision: keep this plan strictly scoped to roadmap 3.4.2 and defer 3.4.3
  provenance reporting endpoints.
  Rationale: roadmap sequencing separates worker behaviour from admin reporting
  concerns.
  Date/Author: 2026-02-26 / Codex.

- Decision: use a four-agent ownership model with explicit merge order.
  Rationale: reduces cross-layer drift and keeps port-first boundaries intact.
  Date/Author: 2026-02-26 / Codex.

- Decision: require full gate logs captured through `tee` under `/tmp` as part
  of completion criteria.
  Rationale: durable gate evidence is required for truthful completion status.
  Date/Author: 2026-02-26 / Codex.

- Decision: keep worker policy (quota + circuit + retry) inside a domain-owned
  `OverpassEnrichmentWorker` and expose only source/metrics/persistence
  boundaries through ports.
  Rationale: preserves hexagonal boundaries while allowing adapter-specific
  implementations for HTTP and Prometheus.
  Date/Author: 2026-02-26 / Codex.

## Outcomes & Retrospective

Shipped:

- Domain ports for Overpass source and enrichment metrics.
- Domain-owned `OverpassEnrichmentWorker` with semaphore admission, daily quota
  checks, retry backoff with jitter, and circuit breaker transitions.
- Outbound Overpass HTTP adapter and Prometheus enrichment metrics adapter.
- `rstest` unit coverage for happy/unhappy/edge policy paths.
- `rstest-bdd` behavioural coverage using embedded PostgreSQL.
- Architecture documentation updates for 3.4.2 design decisions.
- Roadmap closure for item 3.4.2.

Changes versus initial plan:

- Circuit and quota edge assertions were concentrated in unit tests for
  determinism, while BDD scenarios focused on persisted behaviour and
  integration outcomes.
- Worker runtime composition remains adapter-ready without introducing a full
  queue runner mode in this milestone.

Risk handling:

- Module-size lint constraints were resolved by splitting worker test and
  runtime support into dedicated submodules.
- Clippy argument-count violations were resolved with typed dependency bundles
  (`OverpassEnrichmentWorkerPorts`, `OverpassEnrichmentWorkerRuntime`,
  `OverpassHttpIdentity`).

Gate evidence:

- `/tmp/check-fmt-wildside-backend-3-4-2-overpass-enrichment-workers.out`
- `/tmp/lint-wildside-backend-3-4-2-overpass-enrichment-workers.out`
- `/tmp/test-wildside-backend-3-4-2-overpass-enrichment-workers.out`

Follow-up scope:

- Roadmap 3.4.3 enrichment provenance persistence and admin reporting endpoints
  remain pending by design.
