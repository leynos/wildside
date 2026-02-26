# Audit user-state schema coverage and migration needs (roadmap 3.5.1)

This Execution Plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises &
Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must be kept up
to date as work proceeds.

Status: COMPLETE

There is no `PLANS.md` in this repository, so this file is the authoritative
plan for roadmap item 3.5.1.

Implementation started after explicit approval and is now complete.

## Purpose / big picture

Roadmap task 3.5.1 requires a concrete audit of schema and persistence
coverage for login, users, profile, and interests, then a documented decision
on whether new migrations are required for profile and interests storage,
revision tracking, and stale-write conflict handling.

After this work lands, maintainers can point to one auditable source of truth
showing:

- what already exists in schema and adapters;
- what remains fixture-backed and why;
- whether migrations are required now, optional, or deferred with rationale;
- how those findings unblock tasks 3.5.2 through 3.5.6.

Observable success criteria:

- A reproducible audit path exists behind a domain port with outbound adapter
  implementation details confined to persistence modules.
- Audit output captures login/users/profile/interests coverage and explicit
  migration decisions for profile storage, interests storage, revision
  tracking, and conflict handling.
- Unit tests (`rstest`) and behavioural tests (`rstest-bdd`) cover happy,
  unhappy, and edge cases for audit logic and DB-backed execution.
- `docs/wildside-backend-architecture.md` records design decisions taken for
  this audit.
- `docs/backend-roadmap.md` marks 3.5.1 done only after audit docs, tests,
  and gates are all complete.
- `make check-fmt`, `make lint`, and `make test` pass with captured logs.

## Constraints

- Scope is strictly roadmap task 3.5.1. Do not implement 3.5.2 through 3.5.6
  in this change.
- Preserve the hexagonal dependency rule:
  - domain must not import inbound or outbound modules;
  - inbound must not import outbound modules directly;
  - outbound must not import inbound modules.
- Keep persistence details in outbound adapters. Any schema inspection or SQL
  access must happen through driven ports defined in domain modules.
- Keep inbound handlers and server wiring unchanged for 3.5.1 except where
  needed to expose or run the audit workflow safely.
- Do not add new external crates for this task.
- Follow existing testing guidance:
  - `rstest` fixtures for unit and repository-style tests;
  - `rstest-bdd` scenarios for behavioural outcomes;
  - `pg-embedded-setup-unpriv` for local PostgreSQL-backed tests.
- Keep markdown wrapped at 80 columns, except headings, tables, and code
  blocks.

## Tolerances (exception triggers)

- Scope tolerance: if completing 3.5.1 requires implementing DB-backed runtime
  replacement for `LoginService`, `UsersQuery`, `UserProfileQuery`, or
  `UserInterestsCommand`, stop and escalate because that is 3.5.2 and 3.5.3
  scope.
- Interface tolerance: if existing public HTTP request or response contracts
  must change, stop and escalate.
- Churn tolerance: if required changes exceed 14 files or 900 net lines, stop
  and split into a refined milestone plan.
- Validation tolerance: if `make check-fmt`, `make lint`, or `make test`
  fails after three fix cycles, stop and record evidence with options.
- Environment tolerance: if embedded PostgreSQL setup fails repeatedly under
  default settings, stop and escalate with captured bootstrap logs.
- Ambiguity tolerance: if profile or interests migration decisions depend on
  unresolved product constraints not represented in current docs, stop and
  present explicit decision options.

## Risks

- Risk: interests state currently appears in two schema shapes
  (`user_preferences.interest_theme_ids` and `user_interest_themes`), which
  can produce contradictory migration conclusions.
  Severity: high.
  Likelihood: high.
  Mitigation: include a decision checkpoint that selects one canonical
  persistence path for 3.5.x planning and document the rationale.

- Risk: preferences already implement revision mismatch handling, but dedicated
  interests endpoints and domain types do not carry revision semantics.
  Severity: high.
  Likelihood: medium.
  Mitigation: record whether interests conflict handling should reuse the
  preferences revision model or require dedicated schema and port updates.

- Risk: audit work accidentally leaks persistence concerns into domain or
  inbound layers.
  Severity: high.
  Likelihood: medium.
  Mitigation: add or update architecture guardrail tests and keep all schema
  probing behind domain-driven ports with outbound implementations.

- Risk: fixture-backed runtime wiring can obscure real schema coverage if the
  audit does not explicitly separate runtime wiring status from schema status.
  Severity: medium.
  Likelihood: high.
  Mitigation: report runtime wiring and schema coverage as separate axes in
  the audit output.

## Agent team

Implementation uses a four-agent team for design and delivery.

- Agent A: schema and migration audit matrix ownership.
  Owns:
  - schema inventory and mapping logic;
  - migration-decision rubric and generated audit documentation;
  - references to migration files and Diesel schema declarations.

- Agent B: domain and port contract ownership.
  Owns:
  - domain audit model types and decision enums;
  - driven port trait definitions for audit data collection;
  - domain service or use-case orchestration for audit execution.

- Agent C: outbound adapter and integration ownership.
  Owns:
  - persistence adapter implementation for audit data retrieval;
  - PostgreSQL-backed test harness setup and repository-level checks;
  - adapter guardrail alignment for hexagonal boundaries.

- Agent D: tests and docs closure ownership.
  Owns:
  - `rstest` unit tests for happy, unhappy, and edge decision paths;
  - `rstest-bdd` behavioural scenarios and step implementations;
  - architecture decision updates and roadmap checkbox update.

Coordination rules:

- Each agent edits only owned files and ignores unrelated concurrent edits.
- Merge order is B -> C -> A -> D so contracts stabilize before adapter and
  docs outcomes.
- Required tests run at each merge boundary for touched scope.
- Final full gate run happens once all changes merge and before roadmap
  checkbox closure.

## Progress

- [x] (2026-02-26 17:06Z) Confirmed branch context and repository instructions.
- [x] (2026-02-26 17:06Z) Loaded required skills: `execplans`,
      `hexagonal-architecture`, `leta`, and `grepai`.
- [x] (2026-02-26 17:06Z) Collected roadmap 3.5.1 scope and dependencies on
      3.5.2 to 3.5.6.
- [x] (2026-02-26 17:06Z) Ran explorer-agent audit for architecture
      boundaries, testing expectations, and current schema coverage gaps.
- [x] (2026-02-26 17:06Z) Drafted this ExecPlan at
      `docs/execplans/backend-3-5-1-audit-schema-coverage.md`.
- [x] (2026-02-26 17:10Z) Implementation approved by user.
- [x] (2026-02-26 17:23Z) Domain audit contracts implemented in
      `backend/src/domain/user_state_schema_audit.rs` using
      `SchemaSnapshotRepository`.
- [x] (2026-02-26 17:23Z) Unit and behavioural tests added and passing:
      `user_state_schema_audit_tests` and
      `backend/tests/user_state_schema_audit_bdd.rs`.
- [x] (2026-02-26 17:37Z) Added canonical audit report document:
      `docs/user-state-schema-audit-3-5-1.md`.
- [x] (2026-02-26 17:37Z) Updated architecture decision log in
      `docs/wildside-backend-architecture.md`.
- [x] (2026-02-26 17:37Z) Marked roadmap item 3.5.1 done in
      `docs/backend-roadmap.md`.
- [x] (2026-02-26 17:41Z) Final gates executed and evidence captured:
      `make check-fmt`, `make lint`, and
      `NEXTEST_TEST_THREADS=1 make test` (embedded Postgres contention
      mitigation).

## Surprises & discoveries

- Observation (2026-02-26): runtime wiring for login, users, profile, and
  interests is still fixture-backed in `state_builders`, even when DB pool
  configuration exists.
  Evidence: `backend/src/server/state_builders.rs`.
  Impact: schema coverage and runtime port persistence parity are distinct
  concerns and must be reported separately.

- Observation (2026-02-26): interests persistence appears in both
  `user_preferences` and `user_interest_themes` models.
  Evidence: migration and Diesel schema audit from explorer findings.
  Impact: 3.5.1 must produce a canonical storage recommendation before 3.5.3
  and 3.5.4 can be implemented safely.

- Observation (2026-02-26): preferences already have revision mismatch and
  conflict mapping, while dedicated interests paths do not expose equivalent
  revision semantics.
  Evidence: `preferences_service` and repository tests versus
  `user_interests` domain and HTTP contracts.
  Impact: migration and API-decision notes must distinguish existing coverage
  from missing coverage.

## Decision log

- Decision: represent audit output as a domain-level report model with explicit
  coverage and migration enums (`LoginSchemaCoverage`,
  `EntitySchemaCoverage`, `InterestsStorageCoverage`, and
  `MigrationDecision`).
  Rationale: allows deterministic unit testing of decisions without coupling
  tests to SQL or documentation formatting.
  Date/Author: 2026-02-26 / Codex.

- Decision: treat this roadmap item as complete only when both artefacts exist:
  the audit report and architecture decision notes.
  Rationale: roadmap wording requires documentation of migration need decisions,
  and downstream items depend on those decisions.
  Date/Author: 2026-02-26 / Codex.

- Decision: keep this plan in `DRAFT` until explicit user approval.
  Rationale: the execplans workflow requires approval before implementation.
  Date/Author: 2026-02-26 / Codex.

- Decision: use the existing driven port `SchemaSnapshotRepository` as the
  schema metadata source for 3.5.1 audit logic instead of introducing a new
  persistence-facing port.
  Rationale: reuses established hexagonal seam and keeps 3.5.1 scoped to audit
  behaviour rather than port-surface expansion.
  Date/Author: 2026-02-26 / Codex.

## Context and orientation

Roadmap and design references:

- `docs/backend-roadmap.md` (task 3.5.1 and dependencies on 3.5.2 to 3.5.6).
- `docs/wildside-backend-architecture.md` (hexagonal boundaries, user-state
  persistence design, and documentation update target).
- `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/rstest-bdd-users-guide.md`,
  `docs/pg-embed-setup-unpriv-users-guide.md`,
  `docs/rust-doctest-dry-guide.md`,
  `docs/complexity-antipatterns-and-refactoring-strategies.md`.

Current code anchors for the audit:

- Runtime fixture wiring:
  `backend/src/server/state_builders.rs`.
- Driving ports:
  `backend/src/domain/ports/login_service.rs`,
  `backend/src/domain/ports/users_query.rs`,
  `backend/src/domain/ports/user_profile_query.rs`,
  `backend/src/domain/ports/user_interests_command.rs`.
- Driven persistence and schema:
  `backend/src/domain/ports/user_repository.rs`,
  `backend/src/domain/ports/user_preferences_repository.rs`,
  `backend/src/outbound/persistence/diesel_user_repository.rs`,
  `backend/src/outbound/persistence/diesel_user_preferences_repository.rs`,
  `backend/src/outbound/persistence/schema.rs`,
  `backend/migrations/2025-12-10-000000_create_users/up.sql`,
  `backend/migrations/2025-12-29-000001_create_user_preferences/up.sql`,
  `backend/migrations/2026-02-06-012424_schema_baseline_catalogue_descriptor_user_state/up.sql`.
- Existing conflict semantics reference:
  `backend/src/domain/preferences_service.rs`,
  `backend/tests/diesel_user_preferences_repository.rs`.

## Plan of work

Stage A: baseline audit rubric and scope freeze.

Define an explicit audit matrix with rows for login, users, profile, and
interests, and columns for schema coverage, runtime wiring coverage, revision
tracking coverage, and conflict-handling coverage. Include three-level
migration outcomes (`required`, `not required`, `defer with prerequisite`) and
the evidence rule for each outcome. End this stage only when all matrix rules
are written in code comments or docs and reviewed for downstream relevance to
3.5.2 through 3.5.6.

Stage B: domain contract scaffolding.

Add domain types that represent audit observations and migration decisions, and
define a driven port for collecting persistence facts needed by those domain
types. Keep infrastructure-agnostic types in domain modules and ensure they can
be instantiated in unit tests without a database. End this stage when domain
tests compile and architecture-lint boundaries remain clean.

Stage C: outbound adapter and audit execution path.

Implement outbound adapter(s) that gather schema and adapter coverage facts
from migrations and database metadata. Wire this adapter into an audit command
or service that emits the domain report model. Ensure all SQL, Diesel, and
metadata introspection remains in outbound modules. End this stage when
repository-level integration tests can exercise the audit path with embedded
PostgreSQL.

Stage D: documentation outputs and architecture decision capture.

Generate or hand-author a stable audit report document that references
evidence, matrix outcomes, and migration decisions. Update
`docs/wildside-backend-architecture.md` with explicit 3.5.1 decisions covering:
profile storage path, interests storage path, revision model, and stale-write
conflict handling approach. End this stage when docs are cross-linked and
internally consistent.

Stage E: behavioural verification and roadmap closure.

Add `rstest-bdd` scenarios that verify observable audit outcomes across happy,
unhappy, and edge cases, including conflict-model and revision-model outcomes.
Run full gates, capture logs, and update roadmap checkbox 3.5.1 only after all
tests pass and docs are merged.

## Concrete steps

Work from repository root:
`/data/leynos/Projects/wildside.worktrees/backend-3-5-1-audit-schema-coverage`.

Use a branch-safe log suffix:

```bash
BRANCH_SAFE="$(git branch --show-current | tr '/' '-')"
```

Run focused tests while iterating:

```bash
set -o pipefail; cargo test -p backend schema_audit -- --nocapture \
  2>&1 | tee /tmp/test-schema-audit-$(get-project)-${BRANCH_SAFE}.out
set -o pipefail; cargo test -p backend --test '*bdd*' schema_audit \
  2>&1 | tee /tmp/test-schema-audit-bdd-$(get-project)-${BRANCH_SAFE}.out
```

Run embedded PostgreSQL bootstrap where DB-backed tests require it:

```bash
set -o pipefail; cargo run --release --bin pg_embedded_setup_unpriv \
  2>&1 | tee /tmp/pg-bootstrap-$(get-project)-${BRANCH_SAFE}.out
```

Run mandatory final gates:

```bash
set -o pipefail; make check-fmt \
  2>&1 | tee /tmp/check-fmt-$(get-project)-${BRANCH_SAFE}.out
set -o pipefail; make lint \
  2>&1 | tee /tmp/lint-$(get-project)-${BRANCH_SAFE}.out
set -o pipefail; make test \
  2>&1 | tee /tmp/test-$(get-project)-${BRANCH_SAFE}.out
```

Expected success indicators:

```plaintext
make check-fmt  # exit 0
make lint       # exit 0
make test       # exit 0 with new schema audit tests included
```

## Validation and acceptance

Acceptance is behavioural and documentary, not merely compile success.

Tests:

- `rstest` unit coverage verifies audit classification for happy, unhappy, and
  edge matrices.
- Repository or integration tests verify outbound audit adapter behaviour
  against embedded PostgreSQL.
- `rstest-bdd` scenarios verify end-to-end observable audit outcomes and
  migration recommendation states.

Lint and formatting:

- `make check-fmt` succeeds.
- `make lint` succeeds, including architecture guardrails.

Full test suite:

- `make test` succeeds, including new unit and behavioural scenarios.

Documentation and roadmap:

- `docs/wildside-backend-architecture.md` includes 3.5.1 design decisions and
  rationale.
- A dedicated audit report document exists with explicit migration decisions
  and evidence links.
- `docs/backend-roadmap.md` item 3.5.1 is marked complete only after all
  above checks pass.

## Idempotence and recovery

- All commands are re-runnable. Log file names are deterministic per branch
  and action.
- If bootstrap or tests fail due to transient DB state, cleanly stop embedded
  PostgreSQL, rerun setup, and re-run only the failing target first before
  full gates.
- If migration recommendation outcomes change mid-implementation, update
  `Decision Log`, then re-run related unit and behavioural tests before
  continuing.
- Do not mark roadmap progress as done until final gates pass in the same
  change set.

## Artifacts and notes

Primary artefacts expected from implementation:

- ExecPlan updates in this file.
- Audit report document for 3.5.1 decisions.
- Updated architecture decision section in
  `docs/wildside-backend-architecture.md`.
- Updated roadmap checkbox in `docs/backend-roadmap.md`.
- Gate evidence logs in `/tmp/` with branch-safe names.

## Interfaces and dependencies

Implemented domain interfaces remain infrastructure-agnostic:

```rust
pub fn audit_user_state_schema_coverage(
    repository: &dyn SchemaSnapshotRepository,
) -> Result<UserStateSchemaAuditReport, SchemaSnapshotRepositoryError>;

impl UserStateSchemaAuditReport {
    pub fn evaluate(diagram: &SchemaDiagram) -> Self;
}
```

No new third-party dependencies were added. The implementation uses existing
domain ports and existing embedded PostgreSQL test tooling.

## Outcomes & retrospective

- Implemented a domain-backed user-state schema audit operation and surfaced
  explicit migration decisions for profile/interests persistence parity work.
- Added focused coverage in both unit (`rstest`) and behavioural
  (`rstest-bdd`) suites with embedded Postgres.
- Recorded architecture and audit-report decisions in docs and closed roadmap
  item 3.5.1.
- Main follow-up risk remains unchanged: 3.5.3 and 3.5.4 must resolve the
  canonical interests persistence model before wiring DB-backed interests
  adapters.

## Revision note

2026-02-26: Initial draft created from roadmap, architecture, testing guides,
and explorer-agent findings. This establishes scope, tolerances, and staged
delivery for roadmap 3.5.1 and defers implementation until explicit approval.

2026-02-26: Updated after implementation approval and delivery. Added concrete
progress evidence, aligned interface section to shipped code, recorded final
outcomes, and set status to `COMPLETE`.
