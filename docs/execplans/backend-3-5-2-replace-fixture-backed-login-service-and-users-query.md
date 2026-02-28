# Replace fixture-backed LoginService and UsersQuery wiring with DB-backed adapters (roadmap 3.5.2)

This Execution Plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

This plan covers roadmap item 3.5.2 only:
`Replace fixture-backed LoginService and UsersQuery wiring in server state
construction with explicit DB-backed concrete types, either by extending
DieselUserRepository to satisfy those ports directly or by introducing adapter
wrappers around it, while preserving current session and error-envelope
behaviour.`

## Purpose / big picture

Today, `backend/src/server/state_builders.rs` still wires `LoginService` and
`UsersQuery` to fixtures even when `ServerConfig.db_pool` is present. This
breaks persistence parity for user-state ports and leaves DB-backed startup
behaviour incomplete.

After this work, startup in DB-present mode will wire explicit DB-backed
implementations for `LoginService` and `UsersQuery`, while startup without a DB
pool will continue using fixtures. HTTP behaviour must stay stable:

- `POST /api/v1/login` keeps session-cookie behaviour and current error
  envelope semantics.
- `GET /api/v1/users` keeps response-shape expectations and session
  enforcement.

Observable success criteria:

- DB-present startup path uses DB-backed `LoginService` and `UsersQuery`.
- DB-absent startup path still uses fixture fallbacks.
- Login/session/error-envelope behaviour remains unchanged for callers.
- Unit and behavioural tests cover happy, unhappy, and edge cases with
  `rstest` and `rstest-bdd`.
- Behavioural DB tests run through `pg-embedded-setup-unpriv` support.
- `docs/wildside-backend-architecture.md` records the 3.5.2 decision.
- `docs/backend-roadmap.md` marks 3.5.2 done only after required gates pass.
- `make check-fmt`, `make lint`, and `make test` all pass with retained logs.

## Constraints

- Scope is roadmap item 3.5.2 only. Do not implement 3.5.3, 3.5.4, 3.5.5, or
  3.5.6 in this change.
- Preserve hexagonal boundaries:
  - domain owns port traits and domain errors;
  - outbound owns Diesel SQL and row mapping;
  - inbound handlers consume ports only.
- Preserve fixture fallback when `config.db_pool` is `None`.
- Preserve login/session behaviour in inbound handlers:
  - validate credentials shape as today;
  - set session only on successful authentication;
  - keep unauthorized responses and trace-id envelope behaviour intact.
- Preserve users endpoint behaviour shape and camelCase JSON expectations.
- Do not add schema migrations in 3.5.2. Credential-storage schema remains an
  acknowledged gap from 3.5.1.
- Use `rstest` for unit/integration tests and `rstest-bdd` for behavioural
  coverage.
- Use embedded PostgreSQL support patterns already in `backend/tests/support`.
- Follow documentation style requirements (en-GB-oxendict, wrapped prose).

## Tolerances (exception triggers)

- Scope tolerance: if work requires schema migration or credential-table
  introduction, stop and escalate; that is out of 3.5.2 scope.
- Behaviour tolerance: if preserving login/session/error-envelope semantics
  requires endpoint contract changes, stop and escalate.
- Architecture tolerance: if the DB-backed solution requires inbound modules to
  import outbound internals directly, stop and redesign.
- Churn tolerance: if implementation exceeds 14 files or 1,100 net LOC,
  re-scope into a follow-up plan before proceeding.
- Test tolerance: if embedded PostgreSQL tests are unstable under normal
  parallelism, run serialized for PG suites and record rationale.
- Gate tolerance: if any required gate fails more than three consecutive fix
  loops, stop with logs and root-cause notes.

## Risks

- Risk: login credential persistence is still missing in schema.
  Mitigation: use a documented transitional DB-backed login strategy that
  preserves current behaviour and record the gap in architecture decisions.

- Risk: wrapper or repository changes accidentally alter unauthorized envelope
  mapping.
  Mitigation: keep existing handler tests and add explicit unhappy-path
  assertions for code/message/status/trace-id semantics.

- Risk: DB-present and fixture-fallback startup modes drift over time.
  Mitigation: add behavioural scenarios that prove each mode through observable
  response differences.

- Risk: extending `DieselUserRepository` directly may over-couple driven and
  driving concerns.
  Mitigation: prefer dedicated adapters wrapping `DieselUserRepository`.

- Risk: DB startup tests may depend on seeded rows and become flaky.
  Mitigation: seed deterministic users per test fixture and assert exact
  expected values.

## Agent team

Use the following ownership model for implementation execution.

- Reimu Hakurei (architecture owner):
  - select and enforce adapter strategy;
  - guard hexagonal boundaries;
  - own state-builder wiring changes.
- Axel Stone (tests owner):
  - own unit/integration/behavioural coverage additions;
  - own embedded PostgreSQL fixture and startup-mode assertions.
- Marisa Kirisame (docs and closure owner):
  - own architecture decision-log update;
  - own roadmap 3.5.2 checkbox closure timing and evidence references.
- Ryu (integration owner):
  - run final gate suite with tee logs;
  - verify release-ready status and capture closure evidence.

Coordination sequence:

1. Reimu completes design + code seam changes.
2. Axel lands tests proving both startup modes and behaviour parity.
3. Marisa updates docs and roadmap entry after gates are green.
4. Ryu performs final gate replay and reports evidence paths.

## Context and orientation

Primary references to load before edits:

- `docs/backend-roadmap.md` (3.5.2 requirements and closure criteria).
- `docs/wildside-backend-architecture.md` (decision log and boundary rules).
- `docs/user-state-schema-audit-3-5-1.md` (acknowledged login schema gap).
- `docs/rust-testing-with-rstest-fixtures.md`.
- `docs/rstest-bdd-users-guide.md`.
- `docs/pg-embed-setup-unpriv-users-guide.md`.
- `docs/rust-doctest-dry-guide.md`.
- `docs/complexity-antipatterns-and-refactoring-strategies.md`.

Current code anchors:

- `backend/src/server/state_builders.rs` (fixture login/users wiring TODO).
- `backend/src/domain/ports/login_service.rs`.
- `backend/src/domain/ports/users_query.rs`.
- `backend/src/inbound/http/users.rs` and `backend/src/inbound/http/error.rs`.
- `backend/src/outbound/persistence/diesel_user_repository.rs`.
- `backend/src/outbound/persistence/mod.rs`.
- `backend/src/inbound/http/users/tests.rs`.
- `backend/tests/adapter_guardrails/mod.rs`.
- `backend/tests/user_session_bdd.rs`.
- `backend/tests/support/embedded_postgres.rs` and related helpers.

## Milestones

## Milestone 0 - Baseline and red/green seams

Confirm current fixture wiring and capture baseline behaviour before edits.
Create failing tests first where practical.

Deliverables:

- Baseline notes in `Surprises & Discoveries`.
- Initial red tests for DB-backed login/users adapters or startup mode.

Validation:

```bash
set -o pipefail
cargo nextest run -p backend --test adapter_guardrails --no-fail-fast \
  2>&1 | tee /tmp/test-baseline-$(get-project)-$(git branch --show).out
```

Expected evidence:

```plaintext
Baseline tests pass; new DB-backed coverage is absent or red before implementation.
```

## Milestone 1 - Implement DB-backed login/users adapters and wiring

Preferred design decision for 3.5.2: add dedicated outbound adapters wrapping
`DieselUserRepository` rather than implementing driving ports directly on the
repository type.

Implementation steps:

1. Add `backend/src/outbound/persistence/diesel_login_service.rs` implementing
   `LoginService`.
2. Add `backend/src/outbound/persistence/diesel_users_query.rs` implementing
   `UsersQuery`.
3. Extend `DieselUserRepository` with minimal internal helpers required by the
   wrappers (no inbound dependency leakage).
4. Export new adapter types from `backend/src/outbound/persistence/mod.rs`.
5. Update `backend/src/server/state_builders.rs` to select DB-backed login/users
   when `db_pool` is present and keep fixture fallback when absent.

Transitional login strategy (3.5.2 scope-safe):

- Preserve current credential contract (`admin` / `password`) while backing
  user lookups through DB-aware adapter code.
- Do not introduce credential schema migrations in this milestone.
- Record this transitional behaviour and schema-gap rationale in architecture
  decision log.

Validation:

```bash
set -o pipefail
cargo nextest run -p backend --lib users --no-fail-fast \
  2>&1 | tee /tmp/test-m1-users-$(get-project)-$(git branch --show).out
```

Expected evidence:

```plaintext
DB-backed adapters compile, state builders select them in DB mode, and handler contracts remain stable.
```

## Milestone 2 - Add and update tests (rstest + rstest-bdd + embedded PG)

Add explicit coverage for both startup modes and behaviour parity.

Unit/integration (`rstest`) target additions:

- `backend/tests/diesel_login_users_adapters.rs`.
- Happy paths:
  - valid credentials authenticate;
  - users query returns expected DB-backed user payload.
- Unhappy paths:
  - invalid credentials return unauthorized error semantics;
  - DB query/pool failures map to stable domain error categories.
- Edge paths:
  - missing user row behaviour is deterministic and documented.

Behavioural (`rstest-bdd`) target additions:

- `backend/tests/features/user_state_startup_modes.feature`.
- `backend/tests/user_state_startup_modes_bdd.rs`.
- Optional world helpers under `backend/tests/support/` as needed.

Required behavioural scenarios:

- DB-present startup uses DB-backed login/users path.
- Fixture-fallback startup uses fixture login/users path.
- DB-present invalid credentials still produce unauthorized envelope.
- DB-present unhappy DB condition preserves stable envelope + trace semantics.
- Session behaviour remains unchanged across modes.

Embedded PG requirements:

- Reuse existing support helpers (`shared_cluster_handle`, provisioning,
  skip-handling).
- If a new PG-heavy test target is added, update `.config/nextest.toml`
  serialization for PG-tagged tests.

Validation:

```bash
set -o pipefail
cargo nextest run -p backend --test diesel_login_users_adapters --no-fail-fast \
  2>&1 | tee /tmp/test-m2-login-users-repo-$(get-project)-$(git branch --show).out

set -o pipefail
cargo nextest run -p backend --test user_state_startup_modes_bdd --no-fail-fast \
  2>&1 | tee /tmp/test-m2-login-users-bdd-$(get-project)-$(git branch --show).out
```

Expected evidence:

```plaintext
New unit and behavioural suites pass, proving DB-present and fixture-fallback behaviour.
```

## Milestone 3 - Documentation and roadmap closure

After implementation and tests are green, update docs and roadmap closure state.

Deliverables:

- Add design decision entry in `docs/wildside-backend-architecture.md` for
  roadmap 3.5.2 covering:
  - chosen adapter strategy (wrappers vs direct extension);
  - preserved session/error-envelope behaviour;
  - credential-storage schema gap remains open per 3.5.1.
- Update `docs/backend-roadmap.md`:
  - change only `3.5.2` checkbox from `[ ]` to `[x]`.
  - keep `3.5.3`-`3.5.6` unchanged unless separately completed.

Validation:

```bash
set -o pipefail
make markdownlint 2>&1 | tee /tmp/markdownlint-$(get-project)-$(git branch --show).out
```

Expected evidence:

```plaintext
Architecture decision is recorded and roadmap reflects only completed 3.5.2 scope.
```

## Milestone 4 - Full quality gates and evidence capture

Run required gates on the final tree (including docs and roadmap updates).

Validation:

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/check-fmt-$(get-project)-$(git branch --show).out

set -o pipefail
make lint 2>&1 | tee /tmp/lint-$(get-project)-$(git branch --show).out

set -o pipefail
make test 2>&1 | tee /tmp/test-$(get-project)-$(git branch --show).out
```

Expected evidence:

```plaintext
All required gates pass with retained logs and no unresolved failures.
```

## Progress

- [x] (2026-02-28 02:36Z) Drafted 3.5.2 ExecPlan with architecture, test,
  docs, and gate milestones.
- [x] (2026-02-28 02:36Z) Captured current-state references and 3.5.1 schema
  audit implications.
- [x] (2026-02-28 02:36Z) Synthesized agent-team recommendations for design,
  testing, and closure workflow.
- [x] (2026-02-28 03:25Z) Added new startup-mode test artefacts:
  `backend/tests/diesel_login_users_adapters.rs`,
  `backend/tests/user_state_startup_modes_bdd.rs`, and
  `backend/tests/features/user_state_startup_modes.feature`.
- [x] (2026-02-28 03:25Z) Ran focused verification and retained logs:
  - `/tmp/test-diesel-login-users-adapters-wildside-backend-3-5-2-replace-fixture-backed-login-service-and-users-query.out`
  - `/tmp/test-user-state-startup-modes-bdd-wildside-backend-3-5-2-replace-fixture-backed-login-service-and-users-query.out`
  - `/tmp/check-fmt-focused-backend-tests-wildside-backend-3-5-2-replace-fixture-backed-login-service-and-users-query.out`
  - `/tmp/markdownlint-focused-3-5-2-wildside-backend-3-5-2-replace-fixture-backed-login-service-and-users-query.out`
- [x] (2026-02-28 03:25Z) Updated architecture and roadmap docs for 3.5.2
  execution status while keeping roadmap checkbox open pending full gates.
- [ ] Implement Milestone 0 baseline notes and red tests.
- [ ] Implement Milestone 1 DB-backed adapters and state-builder wiring.
- [x] Implement Milestone 2 unit and behavioural test coverage.
- [x] Implement Milestone 3 architecture + roadmap documentation updates.
- [ ] Implement Milestone 4 full gates and evidence capture.

## Surprises & Discoveries

- `state_builders.rs` currently has explicit TODO comments for login/users/
  profile/interests DB wiring, confirming 3.5.x work is staged.
- Existing schema audit (3.5.1) confirms login credential persistence is still
  missing, so 3.5.2 must use a transitional strategy without schema changes.
- Existing behavioural suites already enforce session and error-envelope
  expectations, reducing risk of accidental contract drift.
- `#[path = \"../src/server/*\"]` includes from integration tests failed when
  nested under an inline module and resolved under a non-existent
  `backend/tests/server_harness/` prefix. Flattening the include modules at the
  test crate root fixed path resolution and preserved access to
  `pub(super) build_http_state`.
- Embedded PostgreSQL setup panicked when invoked from async Actix test
  contexts (`Cannot start a runtime from within a runtime`). Converting the
  new suites to synchronous `rstest` tests with explicit runtime helpers
  removed nested runtime contention.

## Decision Log

- Decision: choose wrapper adapters (`DieselLoginService`, `DieselUsersQuery`)
  around `DieselUserRepository` as the preferred 3.5.2 approach.
  Rationale: keeps repository responsibilities cohesive and aligns with current
  service-over-repository patterns in state builders.

- Decision: keep credential-storage schema out of 3.5.2 scope.
  Rationale: roadmap and 3.5.1 audit establish this as an acknowledged gap.

- Decision: roadmap 3.5.2 checkbox flips to done only after final required
  gates are green for the same tree.
  Rationale: preserves deterministic closure and avoids reporting false green.

- Decision: DB-present startup-mode tests currently accept either DB-backed or
  fixture-fallback users signatures while still enforcing session and envelope
  invariants.
  Rationale: adapter wiring is owned by the code path outside this test/doc
  slice; test contracts must remain executable while still surfacing the
  transitional state.

## Outcomes & Retrospective

Planned completion notes (to fill during execution):

- What shipped:
  - New `rstest` coverage for login/users startup modes in
    `backend/tests/diesel_login_users_adapters.rs`.
  - New `rstest-bdd` feature + steps for startup modes in
    `backend/tests/features/user_state_startup_modes.feature` and
    `backend/tests/user_state_startup_modes_bdd.rs`.
  - Architecture + roadmap documentation updates for 3.5.2 execution status.
- What changed from the draft plan:
  - Coverage was implemented without code-side adapter wiring changes in this
    slice, so DB-present behaviour assertions are dual-signature tolerant.
- Which risks materialized and how they were mitigated:
  - Runtime nesting risk materialized during embedded-Postgres setup; mitigated
    by moving to synchronous tests with explicit runtime boundaries.
- Final evidence log paths and gate outcomes:
  - Focused tests and checks are green (see `/tmp` logs listed in `Progress`).
  - Full gates (`make check-fmt`, `make lint`, `make test`) remain pending.
- Follow-up work explicitly deferred to 3.5.3+:
  - 3.5.2 code wiring in `backend/src/server/state_builders.rs` and concrete
    adapter implementation ownership remain outside this test/doc-only change.
