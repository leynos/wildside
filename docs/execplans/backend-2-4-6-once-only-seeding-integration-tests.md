# Add Once-Only Seeding Integration Tests and Demo Data Flow Docs

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

There is no `PLANS.md` in this repository, so this ExecPlan is the sole
execution reference.

## Purpose / Big Picture

Close roadmap task `2.4.6` by proving once-only startup seeding behaviour with
integration-focused tests and by documenting the backend demo data flow. The
observable result is that repeated startup seeding for the same seed key does
not create duplicate rows, failure paths do not create partial rows, and the
architecture document explains the full startup seeding path.

## Constraints

- Keep hexagonal boundaries intact: startup orchestration in inbound modules,
  stateful writes in outbound adapters behind domain ports.
- Use `rstest` for unit tests and `rstest-bdd` v0.4.0 for behavioural tests.
- Use `pg-embedded-setup-unpriv` support for Postgres-backed behavioural runs.
- Do not introduce direct Diesel usage in inbound adapters or domain services.
- Keep documentation updates in en-GB-oxendict spelling and wrapped at 80
  columns.

## Tolerances (Exception Triggers)

- Scope: if more than 9 files or 500 net lines were required, escalate.
- Interfaces: if non-test public APIs needed changes, escalate.
- Dependencies: if a new dependency was needed, escalate.
- Iterations: if quality gates failed after 3 repair loops, escalate.

No tolerances were breached during execution.

## Risks

- Risk: duplicate behavioural assertions could overlap existing seeding tests.
  Severity: medium.
  Likelihood: medium.
  Mitigation: focus new assertions on row-count invariants and no-partial-write
  guarantees.
- Risk: embedded Postgres bootstrap may fail in constrained environments.
  Severity: medium.
  Likelihood: low.
  Mitigation: rely on existing `SKIP-TEST-CLUSTER` handling in shared support.

## Progress

- [x] (2026-02-05) Added startup seeding unit tests for skip and validation
      paths in `backend/src/example_data/startup.rs`.
- [x] (2026-02-05) Strengthened behavioural scenarios in
      `backend/tests/features/example_data_seeding.feature` with row-count
      invariants for once-only and failure paths.
- [x] (2026-02-05) Updated architecture documentation with a dedicated demo
      data flow section and a new design decision entry.
- [x] (2026-02-05) Marked roadmap item `2.4.6` complete.
- [x] (2026-02-05) Ran `make check-fmt`, `make lint`, and `make test` and
      confirmed success.

## Surprises & Discoveries

- Observation: startup seeding already had behavioural scenarios before this
  task, but they did not assert row-count invariants for repeat and error
  flows.
  Evidence: existing `example_data_seeding.feature` only asserted outcome text
  for duplicate and error scenarios.
  Impact: additional count assertions were required to verify no duplicate and
  no partial writes.

## Decision Log

- Decision: extend the existing startup seeding BDD feature file instead of
  adding a new integration harness.
  Rationale: current harness already uses embedded Postgres and has stable
  support fixtures.
  Date/Author: 2026-02-05 / Codex.
- Decision: add a dedicated "Demo data flow" subsection in
  `docs/wildside-backend-architecture.md` near the seeding ports and existing
  seeding decision.
  Rationale: keep flow and decision context in one place for backend engineers.
  Date/Author: 2026-02-05 / Codex.

## Outcomes & Retrospective

Outcome:

- Startup seeding unit coverage now includes disabled seeding, empty seed
  validation, missing database skip handling, and missing registry read errors.
- Behavioural scenarios now assert no duplicate rows after repeat seeding and
  no persisted rows for missing-seed/invalid-registry failures.
- Backend architecture docs now describe the full demo data flow from config
  load to transactional persistence and startup logging.
- `docs/backend-roadmap.md` marks `2.4.6` as done.
- Quality gates passed: `make check-fmt`, `make lint`, and `make test`.

Retrospective:

- The existing embedded Postgres test support was sufficient; incremental
  assertions provided higher confidence without introducing new harness
  complexity.
- Keeping design decisions and operational flow in the architecture document
  reduced ambiguity about where once-only semantics are enforced.

## Context and Orientation

Primary files touched by this work:

- `backend/src/example_data/startup.rs`
- `backend/tests/features/example_data_seeding.feature`
- `docs/wildside-backend-architecture.md`
- `docs/backend-roadmap.md`

Related references:

- `docs/backend-sample-data-design.md`
- `docs/execplans/backend-2-4-5-wire-upstartup-seeding.md`
- `backend/tests/support/example_data_seeding_world.rs`

## Validation and Acceptance

The work is accepted when all of the following hold:

- `make check-fmt` passes.
- `make lint` passes.
- `make test` passes, including startup seeding behavioural scenarios.
- Roadmap item `2.4.6` is checked complete.
- Architecture documentation includes the demo data flow and the design
  decision recorded for this task.
