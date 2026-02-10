# Define catalogue and descriptor domain types (roadmap 3.2.1)

This Execution Plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

There is no `PLANS.md` in this repository, so this ExecPlan is the sole
execution reference.

## Purpose / big picture

Roadmap item 3.2.1 requires first-class domain types for catalogue and
descriptor read models so persistence payload shapes no longer live as
adapter-centric `serde_json::Value` bags. After this work, the backend domain
will own strongly typed representations for:

- `RouteSummary`
- `RouteCategory`
- `Theme`
- `RouteCollection`
- `TrendingRouteHighlight`
- `CommunityPick`
- `Tag`
- `Badge`
- `SafetyToggle`
- `SafetyPreset`

These types will include localisation maps and semantic icon identifiers, and
ingestion-facing operations will consume domain-owned types through domain
ports, keeping persistence details in outbound adapters.

Observable outcome:

- New domain types compile and enforce documented invariants.
- Unit tests (`rstest`) cover happy and unhappy construction paths.
- Behavioural tests (`rstest-bdd`) cover database-backed ingest flows with
  embedded PostgreSQL via `pg-embedded-setup-unpriv`.
- `docs/wildside-backend-architecture.md` records design decisions.
- `docs/backend-roadmap.md` marks task 3.2.1 done once all gates pass.
- `make check-fmt`, `make lint`, and `make test` succeed.

## Constraints

- Preserve hexagonal boundaries:
  - Domain types live in `backend/src/domain/*`.
  - Domain ports in `backend/src/domain/ports/*` expose domain-owned payloads.
  - Outbound adapters in `backend/src/outbound/persistence/*` handle Diesel
    mapping only.
- Keep semantics aligned with:
  - `docs/backend-roadmap.md` section 3.2.1.
  - `docs/wildside-backend-architecture.md` catalogue/descriptor sections.
  - `docs/wildside-pwa-data-model.md` read-model and descriptor shapes.
- Localisation payloads must be modelled as explicit map types rather than raw
  strings or frontend class names.
- Semantic icon identifiers must remain semantic keys (for example,
  `category:nature`) and must not encode presentation class names.
- Tests must use:
  - `rstest` for unit coverage.
  - `rstest-bdd` for behavioural coverage.
  - `pg-embedded-setup-unpriv` test helpers for PostgreSQL-backed behavioural
    flows.
- New or changed modules must remain below 400 lines each; split by feature
  when needed.
- All doc updates follow the documentation style guide and en-GB-oxendict
  spelling.
- Do not mark roadmap item 3.2.1 complete until all quality gates pass.

## Tolerances (exception triggers)

- Scope: if implementation exceeds 16 files or 900 net LOC, stop and reassess.
- API drift: if HTTP endpoint contracts must change in this step, stop and
  defer to roadmap task 3.2.3.
- Dependencies: if a new crate is required, stop and escalate with options.
- Invariant ambiguity: if roadmap/docs conflict on type fields or constraints,
  stop and document options before continuing.
- Validation retries: if `make check-fmt`, `make lint`, or `make test` fails
  more than 3 consecutive fix attempts, stop and escalate.

## Risks

- Risk: introducing strict validation for localisation keys or icon IDs may
  reject legacy fixture data used by ingestion tests.
  Severity: medium
  Likelihood: medium
  Mitigation: codify validation rules in one place, update fixtures
  intentionally, and document rationale in architecture decisions.

- Risk: naming collisions with existing domain symbols (for example
  `InterestThemeId`) may produce unclear module boundaries.
  Severity: medium
  Likelihood: medium
  Mitigation: keep catalogue and descriptor types in dedicated modules and use
  explicit imports/re-exports.

- Risk: behavioural coverage might accidentally test Diesel internals instead
  of port semantics.
  Severity: medium
  Likelihood: low
  Mitigation: structure BDD steps around domain ports and observable DB
  outcomes, not adapter implementation details.

## Progress

- [x] (2026-02-10) Gather roadmap, architecture, testing, and pg-embed guidance
      needed to draft this ExecPlan.
- [x] (2026-02-10) Create initial ExecPlan draft with constraints, tolerances,
      risks, staged work, and quality gates.
- [ ] Implement domain modules and value objects for catalogue/descriptor read
      models.
- [ ] Update domain ports to use new domain-owned types for ingestion payloads.
- [ ] Update outbound persistence adapters and mappings to compile with new
      domain payloads.
- [ ] Add/extend unit tests (`rstest`) for happy/unhappy and edge-case domain
      validation paths.
- [ ] Add/extend behavioural tests (`rstest-bdd`) with embedded PostgreSQL for
      ingest-path coverage.
- [ ] Record design decisions in `docs/wildside-backend-architecture.md`.
- [ ] Mark roadmap item 3.2.1 done in `docs/backend-roadmap.md`.
- [ ] Run and pass `make check-fmt`, `make lint`, and `make test`.
- [ ] Commit gated implementation changes.

## Surprises & discoveries

- Observation (2026-02-10): Existing ingestion ports already model catalogue
  and descriptor payloads, but as ingestion-specific structs under
  `backend/src/domain/ports/*`, not as reusable domain entities.
  Evidence: `backend/src/domain/ports/catalogue_ingestion_repository.rs` and
  `backend/src/domain/ports/descriptor_ingestion_repository.rs`.
  Impact: 3.2.1 should introduce domain entities and then migrate ports to
  consume them to avoid duplicated models.

- Observation (2026-02-10): The `grepai` index failed for one query (`failed to
  decode index: EOF`).
  Evidence: CLI error during semantic search.
  Impact: exact-text fallback (`rg`) is acceptable when semantic index is not
  usable.

## Decision log

- Decision: keep this plan scoped to domain type modelling and ingestion-port
  payload ownership; defer new read ports/endpoints to roadmap tasks 3.2.2 and
  3.2.3.
  Rationale: preserves roadmap sequencing and keeps this change atomic.
  Date/Author: 2026-02-10 / Codex.

- Decision: model localisation and semantic icon identifiers as dedicated domain
  value objects (or aliases/newtypes with validation) rather than raw
  `serde_json::Value`/`String` in domain-facing types.
  Rationale: improves invariants, readability, and testability while keeping
  adapters responsible for JSONB mapping.
  Date/Author: 2026-02-10 / Codex.

## Outcomes & retrospective

Pending implementation.

Completion criteria for retrospective update:

- Confirm the ten required domain types exist with localisation and semantic
  icon support.
- Confirm unit and behavioural coverage captures happy, unhappy, and edge-case
  paths.
- Confirm docs and roadmap are updated and quality gates pass.

## Context and orientation

Primary roadmap and architecture references:

- `docs/backend-roadmap.md` (task 3.2.1).
- `docs/wildside-backend-architecture.md` (catalogue/descriptor persistence
  and design decision log).
- `docs/wildside-pwa-data-model.md` (canonical model shape for localisation,
  icon keys, and read-model entities).

Current backend code relevant to this task:

- `backend/src/domain/mod.rs` (domain exports).
- `backend/src/domain/ports/catalogue_ingestion_repository.rs`.
- `backend/src/domain/ports/descriptor_ingestion_repository.rs`.
- `backend/src/domain/ports/mod.rs`.
- `backend/src/outbound/persistence/diesel_catalogue_ingestion_repository.rs`.
- `backend/src/outbound/persistence/diesel_descriptor_ingestion_repository.rs`.
- `backend/src/outbound/persistence/models/ingestion_rows.rs` (insert row
  mapping support).
- `backend/src/outbound/persistence/schema.rs` (Diesel table declarations).

Existing test patterns to follow:

- Unit tests in domain modules using `rstest`:
  - `backend/src/domain/user/tests.rs`
  - `backend/src/domain/annotations/tests.rs`
- Behavioural database tests using `rstest-bdd` + shared embedded cluster:
  - `backend/tests/schema_baseline_bdd.rs`
  - `backend/tests/features/schema_baseline.feature`
  - `backend/tests/support/pg_embed.rs`
  - `backend/tests/support/mod.rs`

Proposed implementation file layout (final names may be adjusted during coding,
while keeping module boundaries clear and files under 400 lines):

- `backend/src/domain/catalogue/` (new module directory):
  - `mod.rs`
  - `localization.rs` (locale/localized string map types)
  - `icon_identifier.rs` (semantic icon key)
  - `route_summary.rs`
  - `route_category.rs`
  - `theme.rs`
  - `route_collection.rs`
  - `trending_route_highlight.rs`
  - `community_pick.rs`
  - `tests.rs` (module-level tests if practical)
- `backend/src/domain/descriptors/` (new module directory):
  - `mod.rs`
  - `tag.rs`
  - `badge.rs`
  - `safety_toggle.rs`
  - `safety_preset.rs`
  - `tests.rs`

If file count or verbosity becomes excessive, group related types while
maintaining <400 lines per file.

## Plan of work

Stage A: settle domain shape and invariants (no adapter rewiring yet).

- Define shared value objects for:
  - Localisation map shape.
  - Semantic icon identifier.
- Define the ten required domain types with explicit fields aligned to
  `docs/wildside-pwa-data-model.md` and current migration columns.
- Add constructors/builders that enforce chosen invariants (for example:
  non-empty slugs, valid icon identifier format, sane tuple ranges).
- Expose new types through `backend/src/domain/mod.rs`.

Go/no-go check for Stage A:

- `cargo test -p backend domain::` passes for new unit tests before moving to
  port updates.

Stage B: migrate ingestion ports to domain-owned payloads.

- Replace or wrap ingestion structs in:
  - `backend/src/domain/ports/catalogue_ingestion_repository.rs`
  - `backend/src/domain/ports/descriptor_ingestion_repository.rs`
  so trait methods consume the new domain types.
- Keep port names and method contracts stable unless unavoidable.
- Ensure fixture/mock implementations in port modules still compile and remain
  easy to use in tests.

Go/no-go check for Stage B:

- Port trait tests and all compile checks pass without outbound adapter changes
  leaking into domain modules.

Stage C: update outbound adapters and row mappings.

- Update conversion impls in:
  - `backend/src/outbound/persistence/diesel_catalogue_ingestion_repository.rs`
  - `backend/src/outbound/persistence/diesel_descriptor_ingestion_repository.rs`
  to map from new domain types to Diesel insert rows.
- Keep JSONB and array column serialization in outbound adapters only.
- Confirm no inbound/domain module imports outbound modules.

Go/no-go check for Stage C:

- Contract-style DB-backed tests compile and pass for ingestion adapters.

Stage D: test coverage expansion (unit + behavioural).

- Unit tests (`rstest`) in new/updated domain modules:
  - Happy paths: valid constructors/builders and serde round-trips where
    applicable.
  - Unhappy paths: invalid slugs/icon keys/localisation maps or invalid ranges.
  - Edge cases: optional fields (`slug`, `routeId`) and boundary numeric values.
- Behavioural tests (`rstest-bdd`) with embedded PostgreSQL:
  - Add `backend/tests/catalogue_descriptor_domain_types_bdd.rs`.
  - Add `backend/tests/features/catalogue_descriptor_domain_types.feature`.
  - Reuse `backend/tests/support/pg_embed.rs` and
    `backend/tests/support/provision_template_database`.
  - Cover:
    - Happy path: domain payloads persisted via ingestion ports/adapters.
    - Unhappy path: unique-constraint or validation failure surfaces as domain
      port error.
    - Edge path: localisation JSON and icon keys persist intact.

Go/no-go check for Stage D:

- New tests fail before implementation and pass after implementation.

Stage E: documentation and roadmap updates.

- Add a design decision entry in `docs/wildside-backend-architecture.md`
  capturing:
  - Why domain-owned read-model types were introduced.
  - How localisation/icon semantics are validated.
  - Why persistence JSON mapping remains in outbound adapters.
- Mark roadmap task 3.2.1 as done in `docs/backend-roadmap.md` only after all
  tests and gates pass.

Stage F: full validation and commit.

- Run formatting, lint, and full test gates with captured logs.
- Ensure no lint suppressions were added without tight scope and reasons.
- Commit with a descriptive message describing what changed and why.

## Concrete steps

Run from repository root:

1. Implement domain model files and exports:
   - `backend/src/domain/*`
   - `backend/src/domain/ports/*`
   - `backend/src/outbound/persistence/*`

2. Run focused tests while iterating:

    cargo test -p backend domain:: -- --nocapture

    cargo test -p backend --test catalogue_descriptor_domain_types_bdd -- --nocapture

3. Run required quality gates with log capture:

    make check-fmt 2>&1 | tee /tmp/check-fmt-$(get-project)-$(git branch --show).out

    make lint 2>&1 | tee /tmp/lint-$(get-project)-$(git branch --show).out

    make test 2>&1 | tee /tmp/test-$(get-project)-$(git branch --show).out

4. Inspect logs before commit:

    tail -n 40 /tmp/check-fmt-$(get-project)-$(git branch --show).out
    tail -n 40 /tmp/lint-$(get-project)-$(git branch --show).out
    tail -n 40 /tmp/test-$(get-project)-$(git branch --show).out

5. Update docs and roadmap, then commit once all gates pass.

## Validation and acceptance

Functional acceptance:

- Domain defines the 10 required catalogue/descriptor types with localisation
  maps and semantic icon identifiers.
- Ingestion operations still flow through domain ports; outbound adapters remain
  the only place mapping to Diesel/JSONB rows.

Test acceptance:

- Unit tests (`rstest`) cover:
  - Valid construction and serialization behaviour.
  - Invalid constructor inputs (unhappy paths).
  - Boundary and optional-field edge cases.
- Behavioural tests (`rstest-bdd`) cover:
  - PostgreSQL-backed happy path.
  - At least one unhappy persistence/validation path.
  - Localisation/icon fidelity.

Quality-gate acceptance:

- `make check-fmt` passes.
- `make lint` passes.
- `make test` passes.

Documentation acceptance:

- `docs/wildside-backend-architecture.md` includes decision log entry for this
  work.
- `docs/backend-roadmap.md` has `[x] 3.2.1` only after all gates pass.

## Idempotence and recovery

- Domain/model and adapter edits are safe to re-run; rerunning format/lint/test
  should converge without additional mutations.
- Behavioural tests use per-test template databases from embedded PostgreSQL,
  isolating state between runs.
- If a migration-state issue appears in behavioural tests, reprovision via
  existing helpers in `backend/tests/support/pg_embed.rs` rather than mutating
  shared global DB state manually.

## Artifacts and notes

Expected artifacts during implementation:

- New domain modules for catalogue and descriptor entities.
- Updated ingestion ports referencing domain types.
- New/updated unit tests and BDD feature/scenario files.
- Quality gate logs in `/tmp/check-fmt-*.out`, `/tmp/lint-*.out`,
  `/tmp/test-*.out`.

## Interfaces and dependencies

Primary interfaces to exist after implementation:

- Domain entities for all ten roadmap 3.2.1 types.
- Domain value object(s) for localisation map and semantic icon identifier.
- Updated ingestion port signatures in:
  - `crate::domain::ports::CatalogueIngestionRepository`
  - `crate::domain::ports::DescriptorIngestionRepository`

Dependencies:

- No new external crates expected for this task.
- Continue using existing test stack:
  - `rstest`
  - `rstest-bdd`
  - `pg-embedded-setup-unpriv`

## Revision note

- 2026-02-10: Initial plan created for roadmap task 3.2.1 with explicit staged
  implementation, test strategy (`rstest` + `rstest-bdd` + embedded Postgres),
  documentation updates, and quality gates.
