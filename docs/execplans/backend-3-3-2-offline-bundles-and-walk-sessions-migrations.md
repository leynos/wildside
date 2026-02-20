# Deliver offline bundle and walk session migrations with audit and bounds/zoom metadata (roadmap 3.3.2)

This Execution Plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETED (implementation shipped and gates passing)

There is no `PLANS.md` in this repository, so this ExecPlan is the sole
execution reference for roadmap item 3.3.2.

## Purpose / big picture

Roadmap item 3.3.2 requires production migrations for `offline_bundles` and
`walk_sessions` so the domain ports introduced in 3.3.1 can persist data through
outbound adapters without leaking schema details into domain or inbound code.

After this work:

- Diesel migrations create `offline_bundles` and `walk_sessions` with audit
  timestamps and explicit bounds/zoom metadata.
- The persistence schema map (`schema.rs`) includes both new tables and any
  required joins.
- PostgreSQL-backed repository behaviour is validated through domain ports,
  keeping SQL details in outbound adapter/test-adapter layers.
- Test coverage includes `rstest` unit checks and `rstest-bdd` behavioural
  scenarios for happy paths, unhappy paths, and edge cases.
- `pg-embedded-setup-unpriv` remains the local integration harness for
  PostgreSQL lifecycle and deterministic test setup.
- Design decisions are recorded in `docs/wildside-backend-architecture.md`.
- `docs/backend-roadmap.md` item 3.3.2 is marked done only after all required
  gates pass.
- `make check-fmt`, `make lint`, and `make test` pass with captured logs.

Observable success criteria:

- A fresh migrated database contains both new tables with required columns,
  constraints, and indexes.
- Port-level save/find/list/delete flows for offline bundles and walk sessions
  round-trip through PostgreSQL using migrated schema (not ad-hoc table creation).
- Schema-loss/error scenarios map to typed repository errors rather than panic.
- Audit timestamp behaviour is observable (`created_at` initialization,
  `updated_at` monotonic updates on mutation).

## Constraints

- Preserve roadmap scope for **3.3.2 only**:
  - deliver migrations + persistence contract validation;
  - do not implement HTTP endpoints from 3.3.3.
- Keep hexagonal boundaries strict:
  - domain stays free of Diesel/postgres imports;
  - all persistence details remain in outbound adapters or test adapters;
  - inbound layers interact through domain ports only.
- Schema requirements for `offline_bundles` must include:
  - audit timestamps (`created_at`, `updated_at`);
  - bounds metadata and zoom metadata fields;
  - manifest status/progress fields aligned with 3.3.1 domain types.
- Schema requirements for `walk_sessions` must include:
  - audit timestamps (`created_at`, `updated_at`);
  - start/end timing and completion payload compatibility with domain types.
- Test coverage is mandatory:
  - unit tests (`rstest`) for migration SQL/static contracts and edge checks;
  - behavioural tests (`rstest-bdd`) for repository contract behaviour;
  - happy, unhappy, and edge cases explicitly covered.
- Integration tests must use `pg-embedded-setup-unpriv` patterns already used by
  repository suites (`shared_cluster_handle`, `provision_template_database`).
- Keep changed files below 400 lines each; split helpers/modules when needed.
- Update architecture docs with design decisions taken during implementation.
- Mark roadmap 3.3.2 done only after all required gates pass.

## Tolerances (exception triggers)

- Scope tolerance: if implementation needs endpoint changes or inbound route
  wiring, stop and split into a follow-up 3.3.3 task.
- Churn tolerance: if total diff exceeds 18 files or 1,400 net LOC, stop and
  re-scope into smaller milestones.
- Data-shape tolerance: if migration column requirements conflict with 3.3.1
  domain invariants, stop and document alternatives before coding.
- Validation tolerance: if `make check-fmt`, `make lint`, or `make test` fails
  more than three consecutive fix attempts, stop and escalate with logs.
- Runtime tolerance: if embedded-Postgres behavioural tests exhibit persistent
  parallel flakiness, pin `NEXTEST_TEST_THREADS=1` for test runs and record the
  rationale in this plan and commit message.

## Risks

- Risk: migration DDL and domain invariants drift (especially bounds ordering,
  zoom range semantics, and walk completion fields).
  Severity: high.
  Likelihood: medium.
  Mitigation: add static SQL assertions and behavioural round-trip checks through
  the domain constructors.

- Risk: timestamp behaviour is inconsistent (for example, `updated_at` not
  changing on updates).
  Severity: medium.
  Likelihood: medium.
  Mitigation: use trigger-based updates where appropriate and add behavioural
  checks asserting monotonic `updated_at`.

- Risk: existing behavioural tests rely on ad-hoc `CREATE TABLE` test setup,
  which can mask migration defects.
  Severity: high.
  Likelihood: medium.
  Mitigation: switch tests to migrated template databases and keep schema-loss
  checks via explicit `DROP TABLE` operations.

- Risk: JSON/stat payload persistence introduces brittle cast/parsing logic.
  Severity: medium.
  Likelihood: medium.
  Mitigation: centralize serialization in adapter helpers and test malformed-row
  handling as query errors.

## Agent team

Implementation will use a small agent team with explicit ownership:

- Agent A (migrations + schema map): owns migration directory creation,
  `up.sql`/`down.sql`, and `backend/src/outbound/persistence/schema.rs` updates.
- Agent B (adapter + model wiring): owns outbound persistence module updates,
  repository implementation files, and row model additions needed by 3.3.2.
- Agent C (tests + docs): owns `rstest` unit coverage, `rstest-bdd` behavioural
  coverage, architecture decision updates, and roadmap checkbox completion.

Coordination rules:

- Each agent works only within owned files and ignores unrelated concurrent
  edits.
- Merge order is A -> B -> C so tests always validate migrated schema.
- Every merged step re-runs required gates before commit.

## Progress

- [x] (2026-02-20) Confirmed branch context and loaded `execplans`,
      `hexagonal-architecture`, `leta`, and `grepai` skills.
- [x] (2026-02-20) Gathered roadmap, architecture, testing, and pg-embed
      constraints using an explorer agent team.
- [x] (2026-02-20) Inspected current code state: 3.3.1 ports exist, test-only
      PostgreSQL repositories exist, production migration/schema support for
      `offline_bundles`/`walk_sessions` is pending.
- [x] (2026-02-20) Drafted this ExecPlan at
      `docs/execplans/backend-3-3-2-offline-bundles-and-walk-sessions-migrations.md`.
- [x] Implement migration SQL for `offline_bundles` and `walk_sessions` with
      audit + bounds/zoom metadata.
- [x] Update Diesel schema/models/adapters to consume the migrated tables
      through domain ports.
- [x] Add and/or refactor `rstest` unit tests for migration contract coverage.
- [x] Add and/or refactor `rstest-bdd` behavioural tests to validate migrated
      schema behaviour (happy/unhappy/edge).
- [x] Record 3.3.2 design decisions in
      `docs/wildside-backend-architecture.md`.
- [x] Mark roadmap item 3.3.2 as done in `docs/backend-roadmap.md`.
- [x] Run and pass `make check-fmt`, `make lint`, and `make test` with
      branch-scoped logs captured via `tee`.
- [x] Commit implementation in focused, gated commits.

## Surprises & Discoveries

- Observation (2026-02-20): `backend/tests/offline_bundle_walk_session_bdd`
  currently creates `offline_bundles` and `walk_sessions` directly in test code
  (`create_contract_tables`) rather than relying on Diesel migrations.
  Impact: migration defects can be hidden unless tests are moved to migrated
  schema setup.

- Observation (2026-02-20): `backend/src/outbound/persistence/schema.rs` does
  not yet define Diesel `table!` mappings for `offline_bundles` or
  `walk_sessions`.
  Impact: production adapters cannot be implemented without schema map updates.

- Observation (2026-02-20): 3.3.1 already recorded domain decisions and port
  contracts for offline bundles and walk sessions in architecture docs.
  Impact: 3.3.2 documentation should append migration/persistence decisions,
  not duplicate 3.3.1 rationale.

- Observation (2026-02-20): Embedded Postgres cluster startup is flaky under
  default parallel nextest execution in this worktree.
  Impact: `make test` can fail with transient
  `postgresql_embedded::start() failed` panics even when test logic is sound;
  rerunning with `NEXTEST_TEST_THREADS=1` stabilizes the full suite.

## Decision log

- Decision: keep this plan strictly scoped to roadmap 3.3.2 and defer endpoint
  work to 3.3.3.
  Rationale: preserves roadmap sequencing and limits blast radius.
  Date/Author: 2026-02-20 / Codex.

- Decision: validate migration behaviour through domain-port contract tests using
  migrated template databases, replacing ad-hoc test table creation where
  practical.
  Rationale: proves the migration artefacts, not a parallel test schema.
  Date/Author: 2026-02-20 / Codex.

- Decision: execute with an explicit three-agent ownership model (migrations,
  adapters, tests/docs) and staged merge order.
  Rationale: reduces context switching and keeps implementation traceable.
  Date/Author: 2026-02-20 / Codex.

- Decision: run `make test` with `NEXTEST_TEST_THREADS=1` for final gate
  evidence after parallel cluster-start flakiness appears.
  Rationale: `pg-embedded-setup-unpriv` cluster boot can race under default
  nextest parallelism; single-thread execution provides deterministic gate
  outcomes for this branch.
  Date/Author: 2026-02-20 / Codex.

## Context and orientation

Primary references:

- `docs/backend-roadmap.md` (3.3.2 at lines 152-160).
- `docs/wildside-backend-architecture.md` (offline/walk ports and design
  decisions around lines 540-687 and testing strategy around lines 2217-2242).
- `docs/rust-testing-with-rstest-fixtures.md`.
- `docs/rstest-bdd-users-guide.md`.
- `docs/pg-embed-setup-unpriv-users-guide.md`.
- `docs/rust-doctest-dry-guide.md`.
- `docs/complexity-antipatterns-and-refactoring-strategies.md`.

Current implementation anchors:

- Domain ports:
  - `backend/src/domain/ports/offline_bundle_repository.rs`
  - `backend/src/domain/ports/walk_session_repository.rs`
- Domain entities:
  - `backend/src/domain/offline/bundle.rs`
  - `backend/src/domain/walks/session.rs`
- Persistence module:
  - `backend/src/outbound/persistence/mod.rs`
  - `backend/src/outbound/persistence/schema.rs`
  - `backend/src/outbound/persistence/models.rs`
- Existing behavioural suite:
  - `backend/tests/offline_bundle_walk_session_bdd.rs`
  - `backend/tests/offline_bundle_walk_session_bdd/repository_impl.rs`
  - `backend/tests/features/offline_bundle_walk_session.feature`
- Embedded Postgres helpers:
  - `backend/tests/support/embedded_postgres.rs`
  - `backend/tests/support/atexit_cleanup.rs`

## Milestones

### Milestone 1: Migration design and DDL implementation

Create a new migration directory for 3.3.2 and implement `up.sql`/`down.sql`.
The migration must include:

- `offline_bundles` table with:
  - IDs/user/device identity fields;
  - bounds and zoom columns;
  - status/progress columns;
  - `created_at` and `updated_at` audit columns;
  - constraints/indexes supporting owner+device lookups and shape validity.
- `walk_sessions` table with:
  - session/user/route identity fields;
  - start/end timing;
  - stats payload and highlighted POI fields;
  - `created_at` and `updated_at` audit columns;
  - constraints/indexes supporting completion-summary queries.
- trigger wiring for `updated_at` updates (using existing
  `update_updated_at_column()` function).

Implementation notes:

- Prefer additive migration strategy and deterministic down migration ordering.
- Keep SQL comments concise and directly tied to invariants.

### Milestone 2: Diesel schema and persistence surface wiring

Update Diesel schema declarations and persistence exports to match new tables.
Expected updates:

- `backend/src/outbound/persistence/schema.rs`:
  - add `diesel::table!` blocks for both tables;
  - add applicable `joinable!` declarations;
  - update `allow_tables_to_appear_in_same_query!` if needed.
- `backend/src/outbound/persistence/models.rs` (and/or submodules):
  - add row structs and insert/update structs required by repository adapters.
- `backend/src/outbound/persistence/mod.rs`:
  - export new repository adapters when implemented.

### Milestone 3: Repository adapter implementation through domain ports

Implement PostgreSQL/Diesel adapters for `OfflineBundleRepository` and
`WalkSessionRepository` in outbound persistence modules (for example
`diesel_offline_bundle_repository.rs` and `diesel_walk_session_repository.rs`).

Requirements:

- map all DB errors into the appropriate domain repository error enum;
- serialize/deserialize bounds/zoom/stats payloads without leaking DB details to
  the domain;
- enforce deterministic ordering for list operations;
- keep adapter methods thin (translation only; no business-rule logic).

If production adapter rollout is blocked by unresolved architectural choices,
record options in `Decision log` and stop per tolerances.

### Milestone 4: Unit tests with `rstest`

Add/extend unit-level coverage:

- static SQL migration tests (similar to `schema_baseline_unit.rs`) asserting:
  - table creation statements;
  - required audit/bounds/zoom columns;
  - key constraints and indexes;
  - down migration reversibility expectations.
- adapter-focused unit checks for mapping/serialization edge cases where pure
  function coverage is feasible.

Edge cases to include at unit level:

- invalid bounds cardinality;
- out-of-range zoom values from rows;
- negative/overflow size values;
- invalid/malformed stats JSON mapping failures.

### Milestone 5: Behavioural tests with `rstest-bdd`

Refactor/add behavioural suites so they validate migrated schema behaviour via
embedded Postgres, not ad-hoc table bootstrap.

Scenarios must cover:

- happy path:
  - save/list/find/delete for offline bundles;
  - save/find/list-completion for walk sessions.
- unhappy path:
  - dropped-table/query-error mapping for each repository.
- edge path:
  - anonymous owner filtering;
  - completion summaries include only completed sessions and correct ordering;
  - timestamp update behaviour after upsert/update.

Fixture guidance:

- use `#[fixture]` names consistently (or `#[from(...)]` explicitly) per
  `rstest-bdd` rules;
- keep shared mutable world state either as `&mut` world or clearly scoped
  synchronization wrappers;
- continue using shared cluster/template database helpers from
  `backend/tests/support`.

### Milestone 6: Documentation updates

Update architecture and roadmap documents in the same feature branch:

- `docs/wildside-backend-architecture.md`:
  - add a design decision entry for 3.3.2 migration shape and audit strategy;
  - note how adapter contract tests prove port semantics against migrated schema.
- `docs/backend-roadmap.md`:
  - switch 3.3.2 from `[ ]` to `[x]` only after all gates pass.

### Milestone 7: Validation, commit gates, and evidence

Run full gates with log capture:

    set -o pipefail
    BRANCH_SAFE="$(git branch --show | tr '/' '-')"
    PROJECT="$(get-project 2>/dev/null || basename "$PWD")"
    make check-fmt 2>&1 | tee "/tmp/check-fmt-${PROJECT}-${BRANCH_SAFE}.out"
    make lint 2>&1 | tee "/tmp/lint-${PROJECT}-${BRANCH_SAFE}.out"
    make test 2>&1 | tee "/tmp/test-${PROJECT}-${BRANCH_SAFE}.out"

If embedded-Postgres flakes under parallel runs:

    NEXTEST_TEST_THREADS=1 make test 2>&1 | tee "/tmp/test-${PROJECT}-${BRANCH_SAFE}.out"

Commit policy:

- Commit each logical milestone once gates pass for that milestone.
- Keep commit messages imperative and scoped.
- Do not include unrelated file churn.

## Outcomes & retrospective

Shipped versus plan:

- Delivered migration DDL for `offline_bundles` and `walk_sessions` with audit
  timestamps, bounds/zoom metadata, constraints, and query-supporting indexes.
- Added production outbound adapters
  (`DieselOfflineBundleRepository`, `DieselWalkSessionRepository`) and Diesel
  row models while keeping persistence details behind domain ports.
- Added integration coverage for both adapters against embedded PostgreSQL and
  retained behavioural (`rstest-bdd`) contract coverage for offline/walk flows.
- Recorded architecture design decisions for 3.3.2 and marked roadmap item
  3.3.2 complete.

Gate outcomes and evidence:

- `make check-fmt` passed.
  Log: `/tmp/check-fmt-backend-3-3-2-offline-bundles-and-walk-sessions-migrations-backend-3-3-2-offline-bundles-and-walk-sessions-migrations.out`
- `make lint` passed.
  Log: `/tmp/lint-backend-3-3-2-offline-bundles-and-walk-sessions-migrations-backend-3-3-2-offline-bundles-and-walk-sessions-migrations.out`
- Initial `make test` failed because of transient embedded-cluster startup
  errors under parallel nextest.
  Log: `/tmp/test-backend-3-3-2-offline-bundles-and-walk-sessions-migrations-backend-3-3-2-offline-bundles-and-walk-sessions-migrations.out`
- Retried with `NEXTEST_TEST_THREADS=1 make test`; passed with
  `744 tests run: 744 passed, 1 skipped`.
  Log: `/tmp/test-backend-3-3-2-offline-bundles-and-walk-sessions-migrations-backend-3-3-2-offline-bundles-and-walk-sessions-migrations-threads1.out`

Follow-up tasks:

- 3.3.3 endpoint wiring remains pending (`/api/v1/offline/bundles` and
  `/api/v1/walk-sessions`) and should reuse the adapters shipped here without
  changing domain invariants.
