# Generate ER diagram snapshots for roadmap 3.1.2

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DRAFT

No `PLANS.md` file exists in the repository root at the time of writing. If
one is added, this ExecPlan must be updated to follow it.

## Purpose / Big Picture

Roadmap item 3.1.2 requires traceable entity-relationship (ER) diagram
snapshots generated from the current Diesel migrations and stored with project
documentation. After this change, contributors can run one command to rebuild
ER artefacts from the migration-applied schema, and reviewers can diff the
diagram snapshots in version control whenever schema shape changes.

Success is observable when:

- A deterministic snapshot generator exists in the backend codebase and runs
  against a migration-backed Postgres database.
- Generated ER artefacts are committed under `docs/` alongside architecture
  documentation.
- Unit tests (`rstest`) cover happy, unhappy, and edge-case rendering and file
  output behaviour.
- Behavioural tests (`rstest-bdd`) cover end-to-end snapshot generation against
  embedded Postgres provided by `pg-embedded-setup-unpriv`.
- `docs/wildside-backend-architecture.md` records the design decision for ER
  snapshot generation.
- `docs/backend-roadmap.md` marks item `3.1.2` as done.
- `make check-fmt`, `make lint`, and `make test` all pass.

## Constraints

- Preserve hexagonal boundaries: schema introspection and persistence access
  must be exposed through domain ports, with infrastructure confined to
  outbound adapters.
- Keep snapshot generation deterministic so repeated runs produce stable output
  unless schema changes.
- Use existing workspace dependencies unless unavoidable; avoid introducing new
  crates for simple command execution or string rendering.
- Validate with both unit tests (`rstest`) and behavioural tests
  (`rstest-bdd`) including happy and unhappy paths.
- Behavioural tests must use `pg-embed-setup-unpriv` test utilities for local
  Postgres bootstrapping.
- Update architecture documentation with explicit design decisions.
- Mark roadmap item `3.1.2` complete only after all quality gates succeed.

## Tolerances (Exception Triggers)

- Scope: if implementation exceeds 18 files or 900 net lines changed, stop and
  reassess before continuing.
- Interface: if existing public HTTP or WebSocket contracts would need to
  change, stop and escalate.
- Dependencies: if a new dependency appears necessary, document the reason and
  request approval before adding it.
- Tooling: if rendering requires non-reproducible external tooling beyond
  current workspace tools (`mmdc`/Mermaid), stop and escalate.
- Validation retries: if `make check-fmt`, `make lint`, or `make test` fail
  three times without a clear root cause, stop and record options.

## Risks

- Risk: Mermaid rendering output may vary between environments, causing noisy
  snapshot diffs.
  Severity: medium
  Likelihood: medium
  Mitigation: keep canonical source snapshot as Mermaid text, sort all
  introspection output deterministically, and keep rendered output format
  stable.

- Risk: migration introspection queries can return rows in non-deterministic
  order.
  Severity: high
  Likelihood: medium
  Mitigation: enforce explicit ordering in SQL queries and deterministic
  in-memory sorting before rendering.

- Risk: behavioural tests that invoke rendering binaries can be flaky in CI if
  browser prerequisites are missing.
  Severity: medium
  Likelihood: medium
  Mitigation: separate renderer integration from core schema extraction,
  support a no-render mode for logic tests, and run full pipeline tests with
  current workspace Mermaid tooling.

## Progress

- [x] (2026-02-09 20:51Z) Draft ExecPlan for roadmap item 3.1.2.
- [ ] Implement domain port and service for schema graph extraction.
- [ ] Implement outbound Postgres adapter for migration-backed introspection.
- [ ] Implement inbound snapshot generation command and documentation artefact
      writer.
- [ ] Add and pass `rstest` unit tests for rendering and error paths.
- [ ] Add and pass `rstest-bdd` behavioural tests for end-to-end generation.
- [ ] Update architecture design decisions and ER snapshot references.
- [ ] Mark roadmap item `3.1.2` as done.
- [ ] Pass `make check-fmt`, `make lint`, and `make test`.
- [ ] Commit gated implementation.

## Surprises & Discoveries

- Observation: The architecture document already contains a hand-authored
  Mermaid ER diagram, but no automated snapshot pipeline currently exists.
  Evidence: `docs/wildside-backend-architecture.md` contains static ER content
  under "Catalogue and user state diagrams", and repository search found no ER
  snapshot generator.
  Impact: this feature must introduce first-class generation mechanics and
  traceable artefact storage.

- Observation: workspace tooling already includes Mermaid CLI dependencies and
  browser bootstrap support.
  Evidence: root `package.json` includes `@mermaid-js/mermaid-cli`, and
  `scripts/install-mermaid-browser.mjs` supports Mermaid rendering setup.
  Impact: no new diagram-rendering dependency is expected.

## Decision Log

- Decision: store ER snapshot artefacts under `docs/diagrams/er/` as
  deterministic Mermaid source (`.mmd`) plus a rendered image (`.svg`).
  Rationale: source files remain diffable and reviewable, while rendered
  snapshots are directly consumable in documentation.
  Date/Author: 2026-02-09 / Codex.

- Decision: implement schema extraction as a domain-facing port plus outbound
  Postgres adapter, and keep the CLI as an inbound adapter.
  Rationale: roadmap phase 3 requires schema and ingestion operations to remain
  behind domain ports, preserving hexagonal boundaries.
  Date/Author: 2026-02-09 / Codex.

- Decision: validate full generation via `rstest-bdd` scenarios and keep core
  renderer logic covered by `rstest` unit tests.
  Rationale: unit tests provide fast deterministic checks, while behavioural
  scenarios prove end-to-end execution with embedded Postgres.
  Date/Author: 2026-02-09 / Codex.

## Outcomes & Retrospective

Pending implementation. This section will capture:

- final artefact paths,
- quality-gate outcomes,
- roadmap update confirmation,
- and lessons learned about deterministic schema visualization.

## Context and Orientation

This task extends the schema baseline delivered in roadmap item `3.1.1`.
Current migration and test anchors:

- Migration baseline:
  `backend/migrations/2026-02-06-012424_schema_baseline_catalogue_descriptor_user_state/`.
- Existing baseline tests:
  `backend/tests/schema_baseline_unit.rs`,
  `backend/tests/schema_baseline_bdd.rs`, and
  `backend/tests/features/schema_baseline.feature`.
- Existing hexagonal ports:
  `backend/src/domain/ports/mod.rs`.
- Outbound persistence adapters:
  `backend/src/outbound/persistence/mod.rs`.
- Architecture document with current ER diagram:
  `docs/wildside-backend-architecture.md`.
- Roadmap tracking target:
  `docs/backend-roadmap.md` item `3.1.2`.

Key term definitions used in this ExecPlan:

- ER snapshot: a version-controlled diagram artefact generated from the live
  schema produced by Diesel migrations.
- Migration-backed schema: a temporary Postgres database created from current
  migrations, not from hand-written schema files.
- Deterministic output: identical output bytes for identical schema input.

## Plan of Work

Stage A: define ports and domain rendering workflow (no rendering side effects
yet).

1. Add a domain module that defines the schema graph data model (tables,
   columns, foreign-key relationships) and deterministic Mermaid rendering.
2. Add a new domain port trait for schema introspection from a live database.
3. Add domain errors for extraction and snapshot generation failures, with
   explicit variants for connection, introspection, render, and file output
   failures.

Go/no-go gate for Stage A: unit tests for pure rendering logic pass and compile
without infrastructure dependencies in domain modules.

Stage B: implement outbound adapter and inbound command.

1. Add an outbound adapter in `backend/src/outbound/persistence/` that queries
   `information_schema` and `pg_catalog` against a migration-backed database
   and maps rows into domain schema graph types.
2. Add an inbound command entry point (new binary under `backend/src/bin/`)
   that orchestrates:
   - migration-backed database setup,
   - domain port invocation,
   - Mermaid source generation,
   - optional rendered SVG generation, and
   - writing artefacts to `docs/diagrams/er/`.
3. Add deterministic file naming, for example:
   - `docs/diagrams/er/schema-baseline.mmd`
   - `docs/diagrams/er/schema-baseline.svg`
   and include generation metadata in comments where appropriate.

Go/no-go gate for Stage B: running the new command locally produces stable
artefacts with no manual post-processing.

Stage C: add tests (unit + behavioural).

1. Add `rstest` unit tests for:
   - happy path: ordered schema graph renders expected Mermaid text.
   - unhappy path: invalid output destination returns explicit error.
   - unhappy path: renderer invocation failure maps to domain error.
   - edge case: composite keys and many-to-many join tables render correctly.
2. Add `rstest-bdd` behavioural scenarios and Gherkin feature files to cover:
   - happy path: snapshots are generated from migrated embedded Postgres.
   - unhappy path: missing renderer binary (or forced render failure) yields
     clear failure without partial artefacts.
   - edge case: rerun generation and confirm deterministic output unchanged.
3. Reuse `pg-embed-setup-unpriv` cluster fixtures in `backend/tests/support/`
   so behavioural suites run in local and CI environments.

Go/no-go gate for Stage C: new tests fail before implementation and pass after,
with no flaky or order-dependent assertions.

Stage D: documentation and roadmap completion.

1. Update `docs/wildside-backend-architecture.md` with a design decision entry
   describing the generation flow, storage location, and traceability intent.
2. Link to the generated ER artefacts from the architecture documentation.
3. Mark `docs/backend-roadmap.md` item `3.1.2` as done after all gates pass.
4. Update this ExecPlan `Progress`, `Decision Log`, `Surprises & Discoveries`,
   and `Outcomes & Retrospective` sections with final implementation evidence.

## Concrete Steps

Run all commands from repository root:

1. Implement and iterate with targeted tests:

   ```bash
   set -o pipefail
   timeout 300 cargo test --manifest-path backend/Cargo.toml schema_baseline_unit 2>&1 | tee /tmp/test-$(get-project)-$(git branch --show).out
   ```

   ```bash
   set -o pipefail
   timeout 300 cargo test --manifest-path backend/Cargo.toml schema_baseline_bdd 2>&1 | tee /tmp/test-bdd-$(get-project)-$(git branch --show).out
   ```

2. Generate ER snapshots with the new binary (command name to be finalized
   during implementation):

   ```bash
   set -o pipefail
   timeout 300 cargo run --manifest-path backend/Cargo.toml --bin er-snapshots -- --output docs/diagrams/er 2>&1 | tee /tmp/er-snapshots-$(get-project)-$(git branch --show).out
   ```

3. Run required quality gates:

   ```bash
   set -o pipefail
   timeout 300 make check-fmt 2>&1 | tee /tmp/check-fmt-$(get-project)-$(git branch --show).out
   ```

   ```bash
   set -o pipefail
   timeout 300 make lint 2>&1 | tee /tmp/lint-$(get-project)-$(git branch --show).out
   ```

   ```bash
   set -o pipefail
   timeout 300 make test 2>&1 | tee /tmp/test-$(get-project)-$(git branch --show).out
   ```

## Validation and Acceptance

Acceptance is satisfied when all conditions below hold:

- Running the ER snapshot command against current migrations writes deterministic
  artefacts under `docs/diagrams/er/`.
- Unit tests (`rstest`) pass for happy and unhappy paths plus edge cases.
- Behavioural tests (`rstest-bdd`) pass against embedded Postgres provisioned
  via `pg-embed-setup-unpriv`.
- Architecture documentation includes a dated design decision for ER snapshot
  generation and links to artefacts.
- Roadmap item `3.1.2` is checked as complete.
- `make check-fmt`, `make lint`, and `make test` pass.

## Idempotence and Recovery

- Snapshot generation must be safely re-runnable; reruns should overwrite
  artefacts deterministically without accumulating stale files.
- If rendering fails, the command should return a non-zero exit code and avoid
  committing partial output.
- If behavioural setup fails due to cluster bootstrap issues, follow existing
  `backend/tests/support/pg_embed.rs` retry and skip handling patterns.
- All steps in this plan are restartable from the repository root after fixing
  the failure cause.

## Artifacts and Notes

Expected implementation artefacts:

- Domain/port additions under `backend/src/domain/`.
- Outbound adapter additions under `backend/src/outbound/persistence/`.
- Inbound snapshot command under `backend/src/bin/`.
- New unit and behavioural test files under `backend/tests/`.
- Generated ER artefacts under `docs/diagrams/er/`.
- Documentation updates in:
  - `docs/wildside-backend-architecture.md`
  - `docs/backend-roadmap.md`
  - this ExecPlan.

## Interfaces and Dependencies

Planned interfaces:

- Domain port trait for schema graph extraction from a Postgres connection
  context.
- Domain renderer function that converts schema graph values into Mermaid ER
  syntax.
- Inbound command interface for output path and render mode options.

Dependencies:

- Reuse existing workspace tooling for Mermaid rendering (`@mermaid-js/mermaid-cli`).
- Reuse existing Rust crates already present in backend and test support.
- No new external service dependencies are planned.

## Revision note (required when editing an ExecPlan)

2026-02-09: Initial draft created for roadmap item 3.1.2, including hexagonal
port requirements, deterministic ER snapshot generation strategy, test
requirements (`rstest` + `rstest-bdd` + `pg-embed-setup-unpriv`), documentation
update requirements, and quality gates.
