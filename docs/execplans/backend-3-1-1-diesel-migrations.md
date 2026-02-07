# Deliver Diesel schema baseline for roadmap 3.1.1

This Execution Plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DONE

## Purpose / Big Picture

Roadmap item 3.1.1 requires a concrete Diesel migration baseline that
materializes the backend architecture schema for catalogue, descriptor, and
user-state persistence. After completion, the backend can apply the baseline
schema with composite key constraints and GiST (Generalized Search Tree) / GIN
(Generalized Inverted Index) indexes, while ingestion writes are exposed
through domain ports and outbound adapters.

Observable outcome: the new migration applies on embedded Postgres, schema
objects and indices exist, duplicate composite keys are rejected, new
ingestion ports are present, and `make check-fmt`, `make lint`, and
`make test` all pass.

## Constraints

- Keep hexagonal boundaries: domain ports define contracts, outbound adapters
  implement persistence.
- Materialize roadmap 3.1.1 scope: core spatial schema, catalogue tables,
  descriptor tables, and current user-state tables.
- Align `routes` table to architecture shape (`path`, `generation_params`).
- Keep baseline migrations executable in embedded Postgres test environments
  without requiring PostGIS (PostgreSQL Geographic Information System
  extension).
- Use `rstest` and `rstest-bdd` v0.4.0 for validation coverage.
- Update `docs/wildside-backend-architecture.md` with design decisions.
- Mark roadmap entry `3.1.1` done only when all quality gates pass.

## Tolerances (Exception Triggers)

- Scope: if work exceeds 20 files or 1,200 net lines of code (LOC), pause and
  reassess.
- Application Programming Interface (API) drift: if HTTP contract changes are
  required, stop and escalate.
- Dependencies: no new crates unless unavoidable.
- Validation retries: if a gate fails 3 times without clear root cause, stop
  and record options.

## Risks

- Risk: replacing `routes` columns could break existing tests and adapters.
  Severity: high
  Likelihood: medium
  Mitigation: update affected Diesel schema/models/tests in same change.

- Risk: spatial types and indexes in architecture docs may not map one-to-one
  with embedded Postgres capabilities.
  Severity: high
  Likelihood: medium
  Mitigation: use native `PATH`/`POINT` types plus supported GiST/GIN indexes
  in baseline migration, and document the decision.

- Risk: schema/document mismatch for catalogue fields.
  Severity: medium
  Likelihood: medium
  Mitigation: use `docs/wildside-pwa-data-model.md` as tie-break for shapes.

## Progress

- [x] (2026-02-06) Create migration
      `2026-02-06-012424_schema_baseline_catalogue_descriptor_user_state`.
- [x] (2026-02-06) Update Diesel schema and models for new tables.
- [x] (2026-02-06) Add domain ingestion ports and Diesel adapter scaffolding.
- [x] (2026-02-06) Update existing integration setup for new `routes` schema.
- [x] (2026-02-06) Add static migration contract tests (`schema_baseline_unit`).
- [x] (2026-02-06) Add behavioural schema tests (`schema_baseline_bdd`).
- [x] (2026-02-06) Update architecture document decision log.
- [x] (2026-02-06) Mark roadmap item 3.1.1 as done.
- [x] (2026-02-06) Run and pass `make check-fmt`, `make lint`, and `make test`.
- [x] (2026-02-06) Commit gated changes.

## Surprises & Discoveries

- Existing `routes` insert helpers in integration tests still used legacy
  columns (`request_id`, `plan_snapshot`) and needed direct updates.
- Current backend test support already embeds migrations with template
  databases, so baseline migration verification can be added without new
  harness crates.

## Decision Log

- Decision: apply immediate `routes` shape replacement in baseline migration.
  Rationale: chosen implementation strategy for this task scope.
  Date/Author: 2026-02-06 / Codex.

- Decision: add ingestion repositories as new domain ports now, with Diesel
  upsert adapters and no endpoint adoption yet.
  Rationale: keeps schema and ingestion behind ports as required by phase 3.
  Date/Author: 2026-02-06 / Codex.

- Decision: keep schema baseline compatible with embedded Postgres by using
  native `PATH`/`POINT` types and supported GiST/GIN indexes instead of a hard
  PostGIS (PostgreSQL Geographic Information System extension) dependency.
  Rationale: project quality gates require local test execution via
  `pg-embedded-setup-unpriv`, and that environment does not bundle PostGIS.
  Date/Author: 2026-02-06 / Codex.

## Outcomes & Retrospective

Roadmap item 3.1.1 is implemented with gated verification:

- `make check-fmt` passed.
- `make lint` passed.
- `make test` passed (`589 passed, 1 skipped`).

## Context and Orientation

Primary implementation files:

- `backend/migrations/2026-02-06-012424_schema_baseline_catalogue_descriptor_user_state/*`
- `backend/src/outbound/persistence/schema.rs`
- `backend/src/outbound/persistence/models.rs`
- `backend/src/domain/ports/catalogue_ingestion_repository.rs`
- `backend/src/domain/ports/descriptor_ingestion_repository.rs`
- `backend/src/outbound/persistence/diesel_catalogue_ingestion_repository.rs`
- `backend/src/outbound/persistence/diesel_descriptor_ingestion_repository.rs`
- `backend/tests/schema_baseline_unit.rs`
- `backend/tests/schema_baseline_bdd.rs`
- `backend/tests/features/schema_baseline.feature`
- `docs/wildside-backend-architecture.md`
- `docs/backend-roadmap.md`

## Plan of Work

Stage A: migration and schema alignment.
Stage B: port and adapter scaffolding.
Stage C: behavioural and unit validation.
Stage D: docs update, quality gates, commit.

## Concrete Steps

Run from repository root.

- `make check-fmt`
- `make lint`
- `make test`

## Validation and Acceptance

Done means:

- Migration applies in embedded Postgres.
- Required tables and indexes exist.
- Duplicate composite keys fail with unique violations.
- Ingestion ports/adapters compile and integrate into module exports.
- All quality gates pass.

## Idempotence and Recovery

Migration is reversible via `down.sql`. Template database (DB) strategy allows
rerunning integration/behaviour tests without persistent state conflicts.

## Artifacts and Notes

Quality gate logs are captured via `tee` in `/tmp` before final commit.

## Interfaces and Dependencies

No new dependencies added; existing workspace crates are used.

## Revision note

Created during implementation to satisfy roadmap planning traceability and to
act as the living execution record for branch `backend-3-1-1-diesel-migrations`.
