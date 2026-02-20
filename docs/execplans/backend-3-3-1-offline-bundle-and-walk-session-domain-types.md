# Add offline bundle and walk session domain types and repositories (roadmap 3.3.1)

This Execution Plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

There is no `PLANS.md` in this repository, so this ExecPlan is the sole
execution reference.

## Purpose / big picture

Roadmap item 3.3.1 introduces the offline and walk-completion domain surface so
later migration and HTTP work (3.3.2 and 3.3.3) can depend on stable,
validated domain contracts.

After this work:

- `OfflineBundle` and `WalkSession` are first-class domain types with explicit
  invariants.
- Manifest and completion-summary operations are expressed as domain repository
  ports (`OfflineBundleRepository` and `WalkSessionRepository`).
- Persistence details remain confined to outbound adapters, in line with
  hexagonal architecture dependency direction.
- Unit tests (`rstest`) and behavioural contract tests (`rstest-bdd`) cover
  happy paths, unhappy paths, and edge cases using embedded PostgreSQL via
  `pg-embedded-setup-unpriv`.
- `docs/wildside-backend-architecture.md` records design decisions for this
  roadmap item.
- `docs/backend-roadmap.md` marks 3.3.1 done only after all quality gates pass.
- `make check-fmt`, `make lint`, and `make test` succeed.

Observable success criteria:

- Domain constructors reject invalid bounds/zoom/progress/stat payloads and
  preserve valid values.
- Repository ports compile with fixture adapters and typed errors.
- Behavioural tests demonstrate repository contract behaviour against
  PostgreSQL-backed adapters, including schema-loss error mapping.
- The roadmap checkbox for 3.3.1 is switched to `[x]` in the same change set
  that passes all gates.

## Constraints

- Preserve hexagonal architecture invariants:
  - dependency rule: domain code must not import outbound adapter modules.
  - port ownership: repository traits are defined in
    `backend/src/domain/ports/*`.
  - domain purity: no Diesel/postgres types in domain models.
  - adapter isolation: persistence concerns remain in outbound adapters or test
    adapters only.
- Keep scope bounded to roadmap item 3.3.1:
  - do not add Diesel migrations for `offline_bundles`/`walk_sessions`
    (reserved for 3.3.2).
  - do not add HTTP endpoints (reserved for 3.3.3).
- Align domain shape with architecture and PWA documents:
  - `OfflineBundle` manifest fields include bounds, zoom range, status, and
    progress metadata.
  - `WalkSession` captures start/end times and completion summary data.
- Repository contracts must model two responsibilities explicitly:
  - offline bundle manifests (create/read/list/delete/update progress/status).
  - walk completion summaries (record and retrieve sessions).
- Tests must include:
  - `rstest` unit tests for invariants and error formatting.
  - `rstest-bdd` behavioural scenarios for repository contracts.
  - embedded PostgreSQL provisioning through existing test support helpers.
- Keep files under 400 lines; split modules and test helpers when needed.
- Keep documentation in en-GB-oxendict style and follow repo markdown
  conventions.
- Do not mark roadmap task 3.3.1 complete until all quality gates pass.

## Tolerances (exception triggers)

- Scope tolerance: if implementation exceeds 18 files or 1,200 net LOC, stop
  and reassess decomposition.
- Sequence tolerance: if production-ready Diesel adapters cannot be validated
  without introducing 3.3.2 migration work, pause and escalate with options.
- Contract tolerance: if architecture and PWA docs disagree on required
  `OfflineBundle` or `WalkSession` fields, stop, and document alternatives before
  coding.
- Validation tolerance: if `make check-fmt`, `make lint`, or `make test` fails
  more than three consecutive fix attempts, stop and report with logs.
- Runtime tolerance: if new pg-embedded BDD suite exceeds configured nextest
  slow timeout, adjust `.config/nextest.toml` in-scope and document why.

## Risks

- Risk: domain model drift between `docs/wildside-pwa-data-model.md` and
  `docs/wildside-backend-architecture.md` (for example completion summary
  representation).
  Severity: medium.
  Likelihood: medium.
  Mitigation: anchor constructor fields to both docs and record explicit
  decision entries when reconciling differences.

- Risk: roadmap sequencing (3.3.1 before 3.3.2 migrations) can make persistence
  contract testing awkward.
  Severity: medium.
  Likelihood: medium.
  Mitigation: validate contracts via PostgreSQL-backed test adapters and test
  schema setup in behavioural fixtures, while deferring migration artefacts to
  3.3.2.

- Risk: progress/status validation may be too permissive, allowing invalid
  states to cross the domain boundary.
  Severity: high.
  Likelihood: medium.
  Mitigation: centralize validation constructors and add unit plus BDD
  regression coverage for invalid values.

- Risk: behavioural tests may become flaky if cluster bootstrapping is repeated
  per scenario.
  Severity: medium.
  Likelihood: low.
  Mitigation: reuse `shared_cluster_handle()` and template database cloning
  helpers already used by existing repository BDD suites.

## Progress

- [x] (2026-02-19) Confirmed branch and loaded `execplans`, `hexagonal-architecture`,
      `leta`, and `grepai` skills.
- [x] (2026-02-19) Reviewed roadmap item 3.3.1 and architecture/testing guides
      listed in the request.
- [x] (2026-02-19) Mapped current domain/ports/outbound/test harness patterns
      to anchor this plan in repository reality.
- [x] (2026-02-19) Drafted this ExecPlan.
- [x] (2026-02-20) Implemented `domain::offline` and `domain::walks` modules
      with validated `OfflineBundle` and `WalkSession` domain types.
- [x] (2026-02-20) Added repository ports for manifests and completion
      summaries in `backend/src/domain/ports` with fixture implementations and
      typed errors.
- [x] (2026-02-20) Added/extended unit tests (`rstest`) for happy/unhappy/edge
      domain and port fixture behaviour.
- [x] (2026-02-20) Added behavioural repository contract tests (`rstest-bdd`)
      using `pg-embedded-setup-unpriv` fixtures, including happy/unhappy/edge
      scenarios.
- [x] (2026-02-20) Recorded 3.3.1 design decisions in
      `docs/wildside-backend-architecture.md`.
- [x] (2026-02-20) Marked roadmap item 3.3.1 complete in
      `docs/backend-roadmap.md`.
- [x] (2026-02-20) Ran and passed `make check-fmt`, `make lint`, and
      `make test` with captured logs.
- [x] (2026-02-20) Commit gated implementation changes.

## Surprises & discoveries

- Observation (2026-02-19): `backend/src/domain/mod.rs` currently exports
  catalogue/descriptors/annotations but does not yet contain `offline` or
  `walks` modules.
  Evidence: `backend/src/domain/mod.rs`.
  Impact: 3.3.1 needs new domain module additions plus re-exports.

- Observation (2026-02-19): `backend/src/outbound/persistence/schema.rs`
  currently does not declare `offline_bundles` or `walk_sessions`; roadmap item
  3.3.2 is still pending for migrations.
  Evidence: `backend/src/outbound/persistence/schema.rs`,
  `docs/backend-roadmap.md` section 3.3.
  Impact: repository contract validation must avoid silently expanding into
  migration scope.

- Observation (2026-02-19): Existing BDD repository suites already provide a
  stable pattern for embedded Postgres setup, template database provisioning,
  and schema-loss unhappy paths.
  Evidence: `backend/tests/catalogue_descriptor_read_models_bdd.rs`,
  `backend/tests/ports_behaviour.rs`, `backend/tests/support/embedded_postgres.rs`.
  Impact: reuse these fixtures/patterns rather than introducing new bootstrap
  mechanics.

- Observation (2026-02-20): `rstest-bdd` fixture matching is strict on
  parameter names. A step argument named `_world` does not bind to fixture
  `world`.
  Evidence: failure from
  `backend/tests/offline_bundle_walk_session_bdd.rs` reporting missing fixture
  `_world`.
  Impact: shared step fixtures must use exact fixture names in function
  signatures.

- Observation (2026-02-20): Full-suite embedded-Postgres stability improves
  when local runs set explicit runtime/data directories.
  Evidence: `make test` failures with `postgresql_embedded::setup/start()` were
  resolved by running with `PG_RUNTIME_DIR` and `PG_DATA_DIR` plus
  `PG_TEST_BACKEND=postgresql_embedded`.
  Impact: local gate command for this change uses explicit pg-embed environment
  configuration.

## Decision log

- Decision: keep roadmap 3.3.1 focused on domain types and repository port
  contracts; defer migrations and HTTP surface area to 3.3.2 and 3.3.3.
  Rationale: preserves roadmap sequencing while delivering the domain/port seam
  required by the data-platform foundation.
  Date/Author: 2026-02-19 / Codex.

- Decision: model `OfflineBundle` and `WalkSession` as validated domain entities
  with constructor-level invariants instead of loose DTO structs.
  Rationale: keeps invalid persistence payloads from crossing domain boundaries
  and aligns with existing domain-type patterns from 3.2.1.
  Date/Author: 2026-02-19 / Codex.

- Decision: behavioural contract tests will run against PostgreSQL-backed test
  adapters in `backend/tests/*` and use template database fixtures from
  `pg-embedded-setup-unpriv`.
  Rationale: satisfies the requirement for embedded-Postgres behavioural
  validation while keeping 3.3.2 migration artefacts out of 3.3.1 scope.
  Date/Author: 2026-02-19 / Codex.

- Decision: Keep behavioural repository adapters dependency-light by encoding
  timestamp and JSON payloads through SQL text casts in insert statements and
  parsing PostgreSQL timestamp text in adapter mapping helpers.
  Rationale: avoids adding additional SQL codec dependencies while preserving
  strict domain validation at adapter boundaries.
  Date/Author: 2026-02-20 / Codex.

## Context and orientation

Primary references:

- `docs/backend-roadmap.md` (3.3.1 scope and completion criteria).
- `docs/wildside-backend-architecture.md` (offline/walk domain + port intent).
- `docs/wildside-pwa-data-model.md` (`OfflineBundle`/`WalkSession` shapes).
- `docs/rust-testing-with-rstest-fixtures.md`.
- `docs/rstest-bdd-users-guide.md`.
- `docs/pg-embed-setup-unpriv-users-guide.md`.
- `docs/rust-doctest-dry-guide.md`.
- `docs/complexity-antipatterns-and-refactoring-strategies.md`.

Relevant current code:

- Domain module exports: `backend/src/domain/mod.rs`.
- Domain ports: `backend/src/domain/ports/mod.rs` and existing repository port
  modules.
- Outbound adapter patterns:
  `backend/src/outbound/persistence/diesel_catalogue_repository.rs`,
  `backend/src/outbound/persistence/diesel_descriptor_repository.rs`.
- Integration test harness:
  `backend/tests/catalogue_descriptor_read_models_bdd.rs`,
  `backend/tests/ports_behaviour.rs`,
  `backend/tests/support/embedded_postgres.rs`,
  `backend/tests/support/atexit_cleanup.rs`.

Planned file additions/updates (final names may be adjusted during
implementation):

- Domain models:
  - `backend/src/domain/offline/mod.rs`
  - `backend/src/domain/walks/mod.rs`
  - `backend/src/domain/mod.rs` (module exports/re-exports)
- Domain ports:
  - `backend/src/domain/ports/offline_bundle_repository.rs`
  - `backend/src/domain/ports/walk_session_repository.rs`
  - `backend/src/domain/ports/mod.rs` (exports)
- Unit tests:
  - `backend/src/domain/offline/tests.rs` (or inline `#[cfg(test)]` module)
  - `backend/src/domain/walks/tests.rs` (or inline `#[cfg(test)]` module)
  - `backend/src/domain/ports/tests.rs` (port fixture/error coverage)
- Behavioural tests:
  - `backend/tests/offline_bundle_walk_session_bdd.rs`
  - `backend/tests/features/offline_bundle_walk_session.feature`
  - helper modules under `backend/tests/support/` if required
- Documentation:
  - `docs/wildside-backend-architecture.md`
  - `docs/backend-roadmap.md`

## Milestone sequence

1. Define domain types and invariants.

   Add `domain::offline` and `domain::walks` modules with validated,
   serialization-ready entities reflecting roadmap/PWA contracts.

   Minimum domain coverage:

   - `OfflineBundle` with bundle identity, owner/device metadata, bounds,
     zoom range, estimated size, lifecycle status, and progress.
   - `WalkSession` with session identity, route linkage, temporal bounds,
     primary/secondary stats, and highlighted POI IDs.

   Add focused constructor validation (for example bounds ordering,
   progress range, and non-empty IDs where required) plus unit tests for
   success/failure/edge cases.

2. Add repository ports for manifests and completion summaries.

   Create `OfflineBundleRepository` and `WalkSessionRepository` traits in
   `backend/src/domain/ports`, each with typed error enums using
   `define_port_error!` and fixture adapters for non-persistence unit paths.

   Target contract shape:

   - Offline bundle repository supports listing, upserting, and deleting
     manifests per user/device context.
   - Walk session repository supports saving session completions and reading
     summaries by session/route/user criteria needed by 3.3.3.

   Add rstest coverage for fixture adapters and port error formatting.

3. Add behavioural repository contract tests with embedded PostgreSQL.

   Introduce a new `rstest-bdd` suite that validates port behaviour against
   PostgreSQL-backed adapters in the test crate, reusing:

   - `shared_cluster_handle()` from `backend/tests/support/atexit_cleanup.rs`.
   - `provision_template_database()` from
     `backend/tests/support/embedded_postgres.rs`.

   Behavioural scenarios must include:

   - Happy path: create/update/list/delete offline bundle manifests and persist
     and fetch walk completion sessions.
   - Unhappy path: schema-loss or malformed data results mapped to typed
     `Query` errors.
   - Edge path: optional owner/route fields, zero and full progress, empty
     highlight lists, and duplicate/idempotent upsert semantics where relevant.

4. Update architecture decision record and roadmap state.

   Add a dated design-decision entry to
   `docs/wildside-backend-architecture.md` describing:

   - new domain entities and invariants,
   - new repository ports and their responsibilities,
   - confirmation that persistence details remain outbound.

   After all gates pass, mark roadmap task 3.3.1 as done in
   `docs/backend-roadmap.md`.

5. Run quality gates and capture evidence.

   Run required gates with log capture (use `set -o pipefail` and `tee`).
   Suggested command pattern (adapt `<project>` helper as available):

       set -o pipefail
       BRANCH_SANITIZED="$(git branch --show | tr '/' '-')"
       make check-fmt | tee "/tmp/check-fmt-<project>-${BRANCH_SANITIZED}.out"
       make lint | tee "/tmp/lint-<project>-${BRANCH_SANITIZED}.out"
       make test | tee "/tmp/test-<project>-${BRANCH_SANITIZED}.out"

   If a gate fails, fix only in-scope issues, rerun the failed gate, then rerun
   the full gate set before commit.

## Validation matrix

- Unit (`rstest`):
  - domain constructor acceptance and rejection paths.
  - boundary-value checks (progress `0.0`, `1.0`, invalid negative/over-one).
  - bounds and zoom range validity checks.
  - fixture repository default behaviour.
- Behavioural (`rstest-bdd` + embedded Postgres):
  - manifest lifecycle operations persist and read correctly.
  - walk session completion summaries round-trip correctly.
  - persistence failures map to typed query errors.
  - empty-state retrieval returns empty collections rather than panics.
- Workspace gates:
  - `make check-fmt`
  - `make lint`
  - `make test`

## Outcomes & retrospective

Completed 2026-02-20.

Completion evidence:

- Domain modules and exports added for offline bundle and walk session entities.
- New repository ports (`OfflineBundleRepository`,
  `WalkSessionRepository`) added and exported with fixture and mock support.
- Behavioural repository contracts added under
  `backend/tests/offline_bundle_walk_session_bdd.rs` with embedded PostgreSQL.
- Architecture documentation updated with a dated 3.3.1 design decision.
- Roadmap item 3.3.1 marked complete.
- Quality gates passed with logs:
  - `/tmp/check-fmt-wildside-backend-3-3-1-offline-bundle-and-walk-session-domain-types.out`
  - `/tmp/lint-wildside-backend-3-3-1-offline-bundle-and-walk-session-domain-types.out`
  - `/tmp/test-wildside-backend-3-3-1-offline-bundle-and-walk-session-domain-types.out`

Lessons:

- The `postgres` test adapter path in this repo does not include direct
  `chrono::DateTime<Utc>` and `serde_json::Value` SQL codec support by default.
  Casting through text (`($n::text)::timestamptz` and `($n::text)::jsonb`)
  keeps adapters deterministic and dependency-light.
- Embedded Postgres setup/start flakiness in local full-suite runs is
  mitigated by setting explicit runtime/data paths (`PG_RUNTIME_DIR`,
  `PG_DATA_DIR`) and keeping `PG_TEST_BACKEND=postgresql_embedded`.
