# Add catalogue and descriptor read repository ports (roadmap 3.2.2)

This Execution Plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT (awaiting explicit approval before implementation)

There is no `PLANS.md` in this repository, so this ExecPlan is the sole
execution reference.

## Purpose / big picture

Roadmap item 3.2.2 requires read-side domain ports for catalogue and descriptor
models, plus PostgreSQL persistence adapters that satisfy the same contract from
inside outbound adapters. This keeps read model persistence details out of
inbound adapters and domain services, preserving the hexagonal boundary defined
in `docs/wildside-backend-architecture.md`.

After this work:

- `CatalogueRepository` and `DescriptorRepository` exist as domain ports.
- Diesel-backed adapters implement both ports.
- Contract tests validate localized JSON payloads map into validated domain
  localization value objects.
- Unit tests (`rstest`) and behavioural tests (`rstest-bdd`) cover happy,
  unhappy, and edge paths using `pg-embedded-setup-unpriv`.
- Backend architecture documentation captures design decisions.
- `docs/backend-roadmap.md` marks 3.2.2 complete.
- `make check-fmt`, `make lint`, and `make test` pass.

Observable success criteria:

- Reading catalogue and descriptor snapshots through the new ports returns
  typed domain entities with correct localization data.
- Persistence faults (for example missing tables) are surfaced as port-level
  `Query` errors.
- Empty-table cases return empty collections rather than failing.

## Constraints

- Preserve hexagonal dependency direction:
  - new read ports live in `backend/src/domain/ports/*`.
  - inbound adapters must consume ports only, not Diesel modules.
  - outbound adapters own Diesel row/query details.
- Keep roadmap scope bounded to 3.2.2:
  - no new HTTP endpoints in this change (those belong to 3.2.3).
  - no schema migration changes unless a blocker requires escalation.
- Use existing domain entities introduced by 3.2.1
  (`RouteSummary`, `RouteCategory`, `Theme`, `RouteCollection`,
  `TrendingRouteHighlight`, `CommunityPick`, `Tag`, `Badge`, `SafetyToggle`,
  `SafetyPreset`, `InterestTheme`).
- Keep localization payload handling typed and validated via
  `LocalizationMap`/`LocalizedStringSet`; no raw `serde_json::Value` crossing
  domain port boundaries.
- Tests must include:
  - unit coverage with `rstest`.
  - behavioural/contract coverage with `rstest-bdd`.
  - embedded PostgreSQL setup via `pg-embedded-setup-unpriv` helpers.
- Keep each code file below 400 lines; split modules when needed.
- Follow en-GB-oxendict spelling in documentation updates.

## Tolerances (exception triggers)

- Scope tolerance: if implementation exceeds 18 files or 1,100 net LOC,
  pause and reassess decomposition.
- API tolerance: if endpoint or response contracts for 3.2.3 are required to
  make 3.2.2 viable, stop and request scope clarification.
- Migration tolerance: if schema changes are required for read ports,
  escalate before creating migrations.
- Ambiguity tolerance: if architecture doc and roadmap disagree on snapshot
  shape (for example single vs list community picks), stop and record options.
- Validation tolerance: if any quality gate fails more than three consecutive
  fix attempts, stop and report with logs.

## Risks

- Risk: Read-port return types may be underspecified in current code.
  Severity: medium.
  Likelihood: medium.
  Mitigation: define explicit snapshot structs in the domain port layer,
  aligned with `docs/wildside-pwa-data-model.md` and architecture guidance.

- Risk: JSONB-to-domain conversion may accept malformed localization payloads.
  Severity: high.
  Likelihood: medium.
  Mitigation: centralize JSON decoding helpers that always pass through
  `LocalizationMap` validation and include negative contract tests.

- Risk: Embedded Postgres behavioural suites may be slow and flaky under
  default test timeouts.
  Severity: medium.
  Likelihood: medium.
  Mitigation: reuse existing shared cluster/template database helpers and keep
  fixture setup deterministic.

- Risk: Table ordering assumptions may leak into snapshot ordering.
  Severity: low.
  Likelihood: medium.
  Mitigation: define explicit SQL ordering in repository queries and assert it
  in tests.

## Progress

- [x] (2026-02-12) Confirmed branch and loaded `execplans`, `leta`, and
      `grepai` skills.
- [x] (2026-02-12) Reviewed roadmap item 3.2.2 plus architecture/testing/
      pg-embed guidance documents.
- [x] (2026-02-12) Mapped existing ingestion ports/adapters and persistence
      test patterns to anchor this plan in current code.
- [x] (2026-02-12) Drafted this ExecPlan.
- [ ] Add read-side domain port modules for catalogue and descriptors.
- [ ] Add Diesel outbound adapters implementing those ports.
- [ ] Add rstest unit coverage for fixtures, mapping helpers, and unhappy
      conversion cases.
- [ ] Add rstest-bdd behavioural contract tests with embedded PostgreSQL,
      including happy/unhappy/edge scenarios.
- [ ] Record 3.2.2 design decisions in
      `docs/wildside-backend-architecture.md`.
- [ ] Mark roadmap item 3.2.2 done in `docs/backend-roadmap.md`.
- [ ] Run and pass `make check-fmt`, `make lint`, and `make test`.
- [ ] Commit the implementation once all gates pass.

## Surprises & Discoveries

- Observation (2026-02-12): 3.2.1 already introduced validated catalogue and
  descriptor domain entities plus ingestion adapters, so 3.2.2 can focus only
  on read-side ports/adapters and read contracts.
  Evidence: `backend/src/domain/catalogue/mod.rs`,
  `backend/src/domain/descriptors/mod.rs`, and ingestion adapters under
  `backend/src/outbound/persistence/`.

- Observation (2026-02-12): Architecture documentation already names
  `CatalogueRepository` and `DescriptorRepository` as driven ports and states
  that `CatalogueRepository` returns an explore snapshot.
  Evidence: `docs/wildside-backend-architecture.md` lines around the "Driven
  ports (repositories)" section.

- Observation (2026-02-12): Existing behavioural coverage pattern for
  persistence contracts uses template databases from shared embedded Postgres
  fixtures and supports skip-on-unavailable behaviour.
  Evidence: `backend/tests/ports_behaviour.rs`,
  `backend/tests/catalogue_descriptor_ingestion_bdd.rs`, and
  `backend/tests/support/pg_embed.rs`.

## Decision Log

- Decision: Keep 3.2.2 scoped to read ports/adapters and contract tests; do
  not implement HTTP endpoints in this change.
  Rationale: The roadmap explicitly reserves endpoint wiring for 3.2.3.
  Date/Author: 2026-02-12 / Codex.

- Decision: Introduce explicit read snapshot structs for port outputs (rather
  than returning many independent vectors) so 3.2.3 can map endpoint responses
  without leaking adapter concerns.
  Rationale: The architecture and PWA model both describe a cohesive explore
  snapshot contract.
  Date/Author: 2026-02-12 / Codex.

- Decision: Contract tests will include JSONB localization assertions in both
  directions (read decode and shape persistence expectations), and include at
  least one malformed-localization unhappy path.
  Rationale: Roadmap text calls out localization payload contracts as a primary
  deliverable.
  Date/Author: 2026-02-12 / Codex.

## Outcomes & Retrospective

Pending implementation.

Expected completion evidence:

- New read ports and adapters compile and are wired through `mod.rs` exports.
- Unit and behavioural tests demonstrate happy/unhappy/edge behaviour.
- Architecture and roadmap docs are updated.
- Quality gates pass and logs are archived.

## Context and orientation

Primary references:

- `docs/backend-roadmap.md` (section 3.2.2).
- `docs/wildside-backend-architecture.md` (PWA alignment and driven ports).
- `docs/wildside-pwa-data-model.md` (explore snapshot and descriptor shapes).
- `docs/rust-testing-with-rstest-fixtures.md`.
- `docs/rstest-bdd-users-guide.md`.
- `docs/pg-embed-setup-unpriv-users-guide.md`.
- `docs/complexity-antipatterns-and-refactoring-strategies.md`.

Current relevant code:

- Domain entities:
  - `backend/src/domain/catalogue/*`
  - `backend/src/domain/descriptors/mod.rs`
  - `backend/src/domain/localization.rs`
- Existing ports:
  - `backend/src/domain/ports/mod.rs`
  - `backend/src/domain/ports/catalogue_ingestion_repository.rs`
  - `backend/src/domain/ports/descriptor_ingestion_repository.rs`
- Existing outbound persistence patterns:
  - `backend/src/outbound/persistence/mod.rs`
  - `backend/src/outbound/persistence/diesel_catalogue_ingestion_repository.rs`
  - `backend/src/outbound/persistence/diesel_descriptor_ingestion_repository.rs`
  - `backend/src/outbound/persistence/models.rs`
  - `backend/src/outbound/persistence/schema.rs`
- Existing contract-test scaffolding:
  - `backend/tests/catalogue_descriptor_ingestion_bdd.rs`
  - `backend/tests/ports_behaviour.rs`
  - `backend/tests/support/pg_embed.rs`
  - `backend/tests/support/embedded_postgres.rs`

Planned target files (subject to small adjustments during implementation):

- New domain read ports:
  - `backend/src/domain/ports/catalogue_repository.rs`
  - `backend/src/domain/ports/descriptor_repository.rs`
  - `backend/src/domain/ports/mod.rs` (exports)
- New persistence adapters:
  - `backend/src/outbound/persistence/diesel_catalogue_repository.rs`
  - `backend/src/outbound/persistence/diesel_descriptor_repository.rs`
  - `backend/src/outbound/persistence/mod.rs` (exports)
  - `backend/src/outbound/persistence/models.rs` and/or
    `backend/src/outbound/persistence/models/*` for read rows
  - `backend/src/outbound/persistence/json_serializers.rs` (read/decode helpers)
- New tests:
  - `backend/tests/diesel_catalogue_repository.rs`
  - `backend/tests/diesel_descriptor_repository.rs`
  - `backend/tests/catalogue_descriptor_read_models_bdd.rs`
  - `backend/tests/features/catalogue_descriptor_read_models.feature`
  - optional helper additions under `backend/tests/support/*`
- Documentation updates:
  - `docs/wildside-backend-architecture.md`
  - `docs/backend-roadmap.md`

## Milestone sequence

1. Define read-side domain ports and snapshot types.

   Add `CatalogueRepository` and `DescriptorRepository` port traits with
   dedicated error enums using `define_port_error!`. Include fixture
   implementations for unit tests and simple wiring tests. Ensure return types
   are domain-owned and model both collection payloads and generated snapshot
   metadata needed downstream.

   Acceptance checks:

   - Ports compile and are re-exported through
     `backend/src/domain/ports/mod.rs`.
   - Fixture implementations have rstest unit coverage.

2. Implement Diesel adapters for new read ports.

   Add read-query adapters in outbound persistence. Query all required tables,
   map rows into domain entities, and convert localization/image JSON into typed
   domain value objects. Constrain ordering deterministically (for example by
   `slug` or `highlighted_at` where appropriate).

   Acceptance checks:

   - Adapter code remains outbound-only and does not import inbound modules.
   - Diesel and pool failures map to port `Connection`/`Query` errors.
   - Localizations and semantic icon keys are decoded via validated domain
     constructors.

3. Add contract tests for localization payloads and error paths.

   Add rstest integration tests for adapter contracts. Cover:

   - happy path reads with multi-locale JSON payloads.
   - unhappy path where malformed localization JSON yields a `Query` (or
     explicit mapping) error.
   - edge path with empty tables returning empty snapshots.

   Add rstest-bdd scenarios backed by embedded PostgreSQL to verify end-to-end
   contract behaviour with real tables and localized payload fixtures.

   Acceptance checks:

   - Behaviour scenarios execute through `#[scenario]` and use pg-embed test
     fixtures.
   - Scenario steps assert user-visible contract semantics, not Diesel internals.

4. Update architecture decision record and roadmap progress.

   Add a 3.2.2 design decision entry to
   `docs/wildside-backend-architecture.md` describing new read ports, adapter
   boundaries, and localization contract guarantees. Mark roadmap task 3.2.2 as
   done only after all tests and gates pass.

   Acceptance checks:

   - Documentation reflects actual final implementation details.
   - `docs/backend-roadmap.md` flips 3.2.2 to `[x]` after validation.

5. Run quality gates and archive logs.

   Use Makefile entry points and capture outputs with `tee`.

   Suggested command sequence:

       BRANCH="$(git branch --show)"
       PROJECT="$(basename "$(git rev-parse --show-toplevel)")"
       make check-fmt 2>&1 | tee "/tmp/check-fmt-${PROJECT}-${BRANCH}.out"
       make lint 2>&1 | tee "/tmp/lint-${PROJECT}-${BRANCH}.out"
       make test 2>&1 | tee "/tmp/test-${PROJECT}-${BRANCH}.out"

   Acceptance checks:

   - All three commands exit successfully.
   - No lint suppressions are introduced without tight scope and rationale.
   - Logs are available for review in `/tmp`.

## Implementation notes for the executing agent

- Keep commits atomic; one logical change per commit, each gated before commit.
- If an unexpected schema requirement appears, stop and escalate before adding
  migrations.
- If embedded Postgres is unavailable in the local environment, preserve
  existing skip behaviour but still run unit and lint/fmt gates.
- Update this ExecPlan `Progress`, `Surprises & Discoveries`, and `Decision
  Log` as implementation proceeds.
