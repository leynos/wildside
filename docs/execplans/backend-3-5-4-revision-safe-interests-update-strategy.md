# Define the revision-safe interests update strategy (roadmap 3.5.4)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises &
Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must be kept up
to date as work proceeds.

Status: IMPLEMENTED; FULL GATES BLOCKED BY ENVIRONMENT

This plan covers roadmap item 3.5.4 only:
`Define and implement the revision-safe interests update strategy (for
example optimistic concurrency via expected revision checks), including the
persistence contract and error mapping for stale-write conflicts.`

## Purpose / big picture

`PUT /api/v1/users/me/interests` currently writes through
`DieselUserInterestsCommand`, but the command hides concurrency by performing
an internal read-modify-write retry loop against `user_preferences`. That
approach can still collapse concurrent edits into a silent last-write-wins
result because callers do not supply an expected revision and the success
payload does not expose the new revision.

After this change, interests updates will follow the same optimistic
concurrency model already used by `PUT /api/v1/users/me/preferences`,
`POST /api/v1/routes/{route_id}/notes`, and
`PUT /api/v1/routes/{route_id}/progress`: the caller supplies an optional
`expectedRevision`, the write is checked against the canonical persisted
revision, and stale writes produce HTTP `409 Conflict` with structured
`expectedRevision` and `actualRevision` details. The canonical storage remains
`user_preferences.interest_theme_ids` plus `user_preferences.revision`; no new
schema work is expected.

Observable success criteria:

- `PUT /api/v1/users/me/interests` accepts an optional `expectedRevision`.
- Successful interests writes return the new revision so clients can issue the
  next safe update without re-reading the full preferences payload.
- When the persisted revision does not match the supplied
  `expectedRevision`, the endpoint returns HTTP `409` with the stable conflict
  envelope used elsewhere in the backend.
- Interests-only updates preserve the rest of the `user_preferences`
  aggregate, especially `safety_toggle_ids` and `unit_system`.
- Unit coverage (`rstest`) and behavioural coverage (`rstest-bdd`) prove
  happy, unhappy, and edge paths.
- Embedded PostgreSQL flows run through `pg-embedded-setup-unpriv`.
- `docs/wildside-backend-architecture.md` records the aggregate-ownership and
  concurrency decisions for interests writes.
- `docs/backend-roadmap.md` marks only 3.5.4 as done after all gates pass.
- `make check-fmt`, `make lint`, and `make test` succeed, with log files
  retained.

## Constraints

- Scope is roadmap item 3.5.4 only. Do not implement roadmap items 3.5.5 or
  3.5.6 in this change.
- Preserve hexagonal boundaries:
  - domain owns the interests write contract and the conflict semantics;
  - outbound owns Diesel queries and persistence error translation;
  - inbound HTTP handlers only parse requests, call ports, and map responses.
- Treat `user_preferences` as the canonical persistence model for interests.
  Do not revive `user_interest_themes` or add a second revision source.
- Do not add a new migration unless implementation proves the 3.5.1 audit
  wrong. If that happens, stop and escalate because it contradicts the current
  audit record.
- Keep `GET /api/v1/users/me/preferences` and
  `PUT /api/v1/users/me/preferences` semantics unchanged.
- Any public API change must be limited to
  `PUT /api/v1/users/me/interests` request/response shape and related OpenAPI
  documentation.
- Preserve session enforcement and stable trace-bearing error envelopes.
- Use `rstest` for focused unit and integration coverage and `rstest-bdd` for
  behaviour coverage.
- Use `pg-embedded-setup-unpriv` for DB-backed local tests.
- Do not add new external dependencies.
- Update documentation in en-GB-oxendict style.

## Tolerances (exception triggers)

- Scope tolerance: if implementation requires changes to more than 16 files or
  roughly 1,400 net lines of code, stop and split follow-up work.
- Interface tolerance: if more than
  `PUT /api/v1/users/me/interests` and the internal interests driving port must
  change, stop and escalate.
- Aggregate tolerance: if the implementation cannot use
  `user_preferences.revision` as the single concurrency source for interests,
  stop and escalate.
- Dependency tolerance: if a new crate is required, stop and escalate.
- Test tolerance: if focused tests or full gates still fail after three repair
  loops, stop and capture evidence.
- Environment tolerance: if embedded PostgreSQL cannot start after verifying
  `/dev/null`, `PG_TEST_BACKEND`, and required helper tooling, stop and record
  the failure details.
- Ambiguity tolerance: if product semantics for backward compatibility versus
  strict `expectedRevision` enforcement remain disputed, stop and present the
  alternatives before implementation continues.

## Risks

- Risk: interests writes are a partial update over the broader
  `user_preferences` aggregate, so they can conflict with full preferences
  writes even when the interest IDs themselves did not overlap.
  Severity: high.
  Likelihood: high.
  Mitigation: document that interests and preferences share one aggregate
  revision, and make tests prove interests updates preserve non-interest fields
  while still bumping the shared revision.

- Risk: changing the interests driving port will touch fixture adapters,
  recording doubles, startup-mode harnesses, and handler tests.
  Severity: medium.
  Likelihood: high.
  Mitigation: update the typed request/response contract first, then fix all
  consumers in a single mechanical sweep before refining behaviour.

- Risk: current callers may rely on the legacy last-write-wins behaviour of
  `/users/me/interests`.
  Severity: high.
  Likelihood: medium.
  Mitigation: make the compatibility break explicit in docs and architecture
  decision records, and keep the full preferences endpoint unchanged so clients
  already using revision-safe writes remain unaffected.

- Risk: embedded PostgreSQL tests may fail for environmental reasons unrelated
  to the feature.
  Severity: medium.
  Likelihood: medium.
  Mitigation: use repo-standard `pg-embedded-setup-unpriv` helpers, retain
  logs, and record the known `/dev/null` and missing-lint-tool failure modes in
  the execution transcript.

## Progress

- [x] (2026-03-13 02:17Z) Reviewed roadmap item 3.5.4, the 3.5.1 audit, the
  3.5.3 closure notes, the current interests adapter, and the required testing
  and architecture guidance.
- [x] (2026-03-13 02:17Z) Drafted this ExecPlan at
  `docs/execplans/backend-3-5-4-revision-safe-interests-update-strategy.md`.
- [x] Approval gate: user approved implementation by requesting the ExecPlan
  to be carried out.
- [x] Convert the interests driving port and HTTP payload/schema to a
  revision-aware contract.
- [x] Implement the DB-backed stale-write strategy and remove hidden
  last-write-wins retry behaviour.
- [x] Add or update `rstest` coverage for unit, adapter, and DB-backed
  conflict semantics.
- [x] Add or update `rstest-bdd` coverage for HTTP-level happy, unhappy, and
  edge flows with embedded PostgreSQL. The suite compiles; execution is
  presently blocked by embedded PostgreSQL bootstrap failure in this
  container.
- [x] Record final design decisions in
  `docs/wildside-backend-architecture.md`. `docs/backend-roadmap.md` remains
  unchanged until every required gate is green.
- [ ] Run doc checks and full repository gates, retaining logs.

## Surprises & discoveries

- Observation: `DieselUserInterestsCommand` already depends on
  `UserPreferencesRepository`, not on a dedicated interests repository.
  Evidence:
  `backend/src/outbound/persistence/diesel_user_interests_command.rs`.
  Impact: 3.5.4 should refine the existing aggregate contract instead of adding
  another persistence abstraction.

- Observation: the current interests command hides concurrent writes behind a
  bounded retry loop and never surfaces the winning revision on success.
  Evidence:
  `backend/src/outbound/persistence/diesel_user_interests_command.rs` and its
  `tests/retry.rs`.
  Impact: the current behaviour is not revision-safe from the caller’s
  perspective and must be replaced, not merely documented.

- Observation: the 3.5.1 audit already established that
  `user_preferences.revision` is sufficient for interests conflict handling and
  that no revision-tracking migration is required.
  Evidence: `docs/user-state-schema-audit-3-5-1.md`.
  Impact: the implementation should stay inside domain/adapter code and tests.

- Observation: the execution environment still recreates `/dev/null` as a
  regular file (`-rw-r--r--`) instead of the expected character device, so
  `pg-embedded-setup-unpriv` fails during PostgreSQL bootstrap before any
  scenario logic runs.
  Evidence: `ls -l /dev/null` reported a regular file on 2026-03-13, and
  `cargo test -p backend --test user_interests_revision_conflicts_bdd` failed
  with repeated `cannot create /dev/null: Permission denied`.
  Impact: DB-backed BDD scenarios compile but cannot be executed to completion
  in this container until the runtime is repaired.

## Decision Log

- Decision: use `user_preferences` as the single aggregate and concurrency
  source for interests writes.
  Rationale: the schema audit already blesses `user_preferences.revision` as
  sufficient, and adding a second interests revision source would recreate the
  parity gap 3.5.x is closing.
  Date/Author: 2026-03-13 / planning team.

- Decision: replace the current raw-parameter interests port with a typed
  request carrying `expected_revision`, and return a success payload that
  includes the new revision.
  Rationale: revision-safe writes need an explicit caller contract; otherwise
  the adapter can only guess and retry.
  Date/Author: 2026-03-13 / planning team.

- Decision: align stale-write error mapping with the existing preferences and
  annotations conflict envelope using `code: revision_mismatch`,
  `expectedRevision`, and `actualRevision`.
  Rationale: clients already consume this shape elsewhere, so reuse lowers
  cognitive load and avoids contract fragmentation.
  Date/Author: 2026-03-13 / planning team.

- Decision: implement this work with an agent team rather than a single
  undifferentiated change stream.
  Rationale: the task spans domain contracts, Diesel persistence, HTTP/OpenAPI,
  BDD flows, and architecture documentation; explicit ownership reduces drift
  between those layers.
  Date/Author: 2026-03-13 / planning team.

## Outcomes & retrospective

Implementation state:

- `PUT /api/v1/users/me/interests` now accepts `expectedRevision` and returns
  the updated interests payload with `revision`.
- `DieselUserInterestsCommand` now follows explicit optimistic concurrency
  semantics over `user_preferences.revision` and no longer retries stale
  writes into silent success.
- stale or omitted revisions on an existing preferences row now surface as
  `409 Conflict` with top-level `code: "conflict"` and nested details containing
  `code: "revision_mismatch"`, `expectedRevision`, and `actualRevision`.
- unit coverage and non-DB behavioural coverage passed locally.
- DB-backed behavioural execution is blocked by the known `/dev/null` bootstrap
  failure in embedded PostgreSQL, so full-gate replay and roadmap closure are
  still pending environment repair.

## Context and orientation

This roadmap item sits immediately after the 3.5.3 adapter-parity work. The
current implementation state is:

- `backend/src/domain/ports/user_interests_command.rs` exposes
  `UserInterestsCommand::set_interests(&UserId, Vec<InterestThemeId>)` with no
  expected revision and no typed request object.
- `backend/src/domain/user_interests.rs` models only `user_id` and
  `interest_theme_ids`; it does not expose a persisted revision.
- `backend/src/inbound/http/users.rs` accepts
  `InterestsRequest { interest_theme_ids }` and returns `UserInterests` from
  `PUT /api/v1/users/me/interests`.
- `backend/src/outbound/persistence/diesel_user_interests_command.rs` performs
  a read-modify-write against `UserPreferencesRepository`, preserving
  `safety_toggle_ids` and `unit_system`, but resolves some races by retrying
  instead of exposing a stale-write contract to the caller.
- `backend/src/domain/ports/user_preferences_repository.rs` and
  `backend/src/outbound/persistence/diesel_user_preferences_repository.rs`
  already implement the canonical optimistic-concurrency persistence contract
  backed by `user_preferences.revision`.
- `docs/wildside-backend-architecture.md` already states that
  `/users/me/interests` is a backward-compatibility endpoint over the broader
  preferences model, while `/users/me/preferences` is the preferred full
  revisioned API.

The important architectural implication is that interests are not a separate
aggregate. They are a projection over the `user_preferences` aggregate. That
means an interests-only write must preserve the rest of the preferences row and
must participate in the same revision sequence as full preferences writes.

## Agent team and ownership

This implementation should be executed by the following agent team. One person
may play multiple roles if needed, but the responsibilities should stay
separate.

- Coordinator agent:
  owns sequencing, keeps this ExecPlan current, enforces tolerances, collects
  gate evidence, and decides when the work is ready to mark roadmap 3.5.4 done.
- Domain contract agent:
  updates `backend/src/domain/user_interests.rs` and
  `backend/src/domain/ports/user_interests_command.rs`, plus any shared domain
  tests, to define the revision-aware interests contract.
- Persistence adapter agent:
  updates `backend/src/outbound/persistence/diesel_user_interests_command.rs`
  and related test support to implement the canonical stale-write strategy over
  `UserPreferencesRepository`.
- HTTP contract agent:
  updates `backend/src/inbound/http/users.rs`,
  `backend/src/inbound/http/schemas.rs`, and handler tests so the HTTP and
  OpenAPI contracts match the new domain port.
- QA agent:
  adds or updates `rstest` and `rstest-bdd` coverage, including embedded
  PostgreSQL scenarios for real stale-write conflicts.
- Documentation agent:
  updates `docs/wildside-backend-architecture.md`,
  `docs/wildside-pwa-data-model.md` if the public payload shape changes, and
  finally `docs/backend-roadmap.md` once all gates pass.

Hand-off order:

1. Domain contract agent lands the typed contract and failing tests.
2. Persistence adapter agent makes the contract pass in the DB-backed path.
3. HTTP contract agent wires the endpoint and OpenAPI schema.
4. QA agent expands behavioural coverage and verifies DB-backed conflict flows.
5. Documentation agent records the decisions and closes the roadmap item.
6. Coordinator agent runs final gates and updates this ExecPlan.

## Plan of work

Stage A: lock the contract and red-state the behaviour.

Start by replacing the implicit interests-write contract with an explicit one.
The recommended shape is:

- add `UpdateUserInterestsRequest` under
  `backend/src/domain/ports/user_interests_command.rs` containing
  `user_id`, `interest_theme_ids`, and `expected_revision: Option<u32>`;
- extend `backend/src/domain/user_interests.rs` so successful interests writes
  expose `revision: u32`;
- keep the response body as interests-focused data rather than returning the
  full `UserPreferences` aggregate.

Before implementing behaviour, add failing tests that lock the intended
semantics:

- unit tests for the domain port fixture and request/response types;
- handler tests proving `expectedRevision` is parsed and HTTP `409` is part of
  the endpoint contract;
- BDD scenarios that currently fail because the endpoint does not reject stale
  revisions and does not return a revision on success.

Do not continue until the failing tests demonstrate the exact missing
behaviour.

Stage B: implement the revision-safe domain and adapter strategy.

Update `backend/src/outbound/persistence/diesel_user_interests_command.rs` so
it no longer resolves stale writes by hidden retries. Instead, it should follow
the same semantic matrix used by `UserPreferencesService::perform_update`,
while still preserving the non-interest fields already stored in
`user_preferences`:

- if no preferences row exists and `expected_revision` is `None`, insert a new
  row with revision `1`;
- if no preferences row exists and `expected_revision` is `Some(n)`, return a
  revision conflict with `actualRevision: 0`;
- if a preferences row exists and `expected_revision` is `None`, return a
  revision conflict because the caller omitted the required concurrency token;
- if a row exists and the current revision differs from
  `expected_revision`, return a revision conflict with the stored revision;
- if the revisions match, write the interests subset while carrying forward
  `safety_toggle_ids` and `unit_system`, and increment the shared revision by
  one.

The adapter may still perform one post-failure re-read only to disambiguate a
repository-level insert race (`save(..., None)` lost to another writer). That
re-read exists to improve error reporting, not to convert a stale write into a
success.

Stage C: wire the HTTP and OpenAPI contract.

Update `backend/src/inbound/http/users.rs` so `InterestsRequest` includes
`expected_revision: Option<u32>`. The handler should build the typed domain
request and return a revisioned interests payload. Add the `409 Conflict`
response to the `utoipa` annotation, update
`backend/src/inbound/http/schemas.rs`, and extend any OpenAPI or schema tests
that currently assume the old interests payload.

All test doubles that implement `UserInterestsCommand` must be updated in the
same stage so the rest of the test harness compiles. The coordinator should
expect changes in:

- `backend/tests/adapter_guardrails/doubles_users.rs`;
- helper defaults in `backend/tests/adapter_guardrails/harness_defaults.rs`;
- any HTTP harnesses that seed interests payloads or assert response JSON.

Stage D: add focused DB-backed and behavioural regression coverage.

Extend the existing unit suite under
`backend/src/outbound/persistence/diesel_user_interests_command/tests/` to
prove:

- successful first write creates revision `1`;
- successful update with matching `expected_revision` increments the revision;
- stale update maps to conflict without retrying into success;
- omitted `expected_revision` against an existing row maps to conflict;
- unrelated `safety_toggle_ids` and `unit_system` are preserved;
- insert-race ambiguity maps to a stable conflict payload rather than an
  internal error.

Add a new embedded-Postgres behavioural suite, preferably under
`backend/tests/user_interests_revision_conflicts_bdd.rs` with a companion
feature file such as
`backend/tests/features/user_interests_revision_conflicts.feature`. This suite
should exercise the real HTTP adapter and DB-backed state wiring for:

- happy path: first interests write succeeds and returns revision `1`;
- happy path: second write with matching `expectedRevision` succeeds and
  returns revision `2`;
- unhappy path: stale `expectedRevision` returns HTTP `409` with stable
  revision details;
- unhappy path: missing `expectedRevision` after a row exists returns HTTP
  `409`;
- edge path: interests write preserves non-interest preference fields across a
  revision bump, observable via `GET /api/v1/users/me/preferences`.

Stage E: document the decisions, close the roadmap item, and replay gates.

Record the design decision in `docs/wildside-backend-architecture.md` that
interests writes are a partial update of the `user_preferences` aggregate and
therefore share the same revision counter and stale-write policy as full
preferences writes. If the public request or response schema changes, update
`docs/wildside-pwa-data-model.md` so the client-facing model stays aligned.
Only after all tests and checks pass should `docs/backend-roadmap.md` mark
3.5.4 done.

## Concrete steps

Run all commands from `/home/user/project`. Use `set -o pipefail` and `tee`
for every meaningful command so the exit code survives truncation and the log
is retained.

1. Capture the current baseline and confirm the old interests contract.

   ```bash
   set -o pipefail
   cargo test -p backend update_interests --lib 2>&1 | tee /tmp/3-5-4-baseline-users-tests.out
   ```

   Expected pre-change signal:

   ```plaintext
   test ...update_interests_rejects_too_many_ids ... ok
   test ... no stale-write conflict coverage yet ...
   ```

2. Add failing unit and behaviour tests for the revision-safe contract.

   ```bash
   set -o pipefail
   cargo test -p backend user_interests --lib 2>&1 | tee /tmp/3-5-4-red-unit.out
   set -o pipefail
   cargo test -p backend --test user_interests_revision_conflicts_bdd \
     2>&1 | tee /tmp/3-5-4-red-bdd.out
   ```

   Expected red-state examples:

   ```plaintext
   thread '...stale interests update...' panicked at 'expected status 409, got 200'
   thread '...returns revision...' panicked at 'expected field revision'
   ```

3. Implement the domain port, adapter, and HTTP contract changes, then rerun
   the focused suites until they pass.

   ```bash
   set -o pipefail
   cargo test -p backend user_interests --lib 2>&1 | tee /tmp/3-5-4-green-unit.out
   set -o pipefail
   cargo test -p backend --test user_interests_revision_conflicts_bdd \
     2>&1 | tee /tmp/3-5-4-green-bdd.out
   ```

   Expected green-state examples:

   ```plaintext
   test ...stale_interests_update_returns_conflict... ok
   test ...first_interests_write_returns_revision_1... ok
   test result: ok.
   ```

4. Run documentation-specific checks required for Markdown edits.

   ```bash
   set -o pipefail
   make fmt 2>&1 | tee /tmp/3-5-4-fmt.out
   set -o pipefail
   make markdownlint 2>&1 | tee /tmp/3-5-4-markdownlint.out
   set -o pipefail
   make nixie 2>&1 | tee /tmp/3-5-4-nixie.out
   ```

5. Run repository-wide gates required before closure.

   ```bash
   set -o pipefail
   make check-fmt 2>&1 | tee /tmp/3-5-4-check-fmt.out
   set -o pipefail
   make lint 2>&1 | tee /tmp/3-5-4-lint.out
   set -o pipefail
   make test 2>&1 | tee /tmp/3-5-4-test.out
   ```

6. Update docs and roadmap only after all gates pass, then append final
   evidence to this ExecPlan.

## Validation and acceptance

The implementation is done only when all of the following are true:

- HTTP behaviour:
  - `PUT /api/v1/users/me/interests` accepts JSON like
    `{ "interestThemeIds": ["..."], "expectedRevision": 1 }`.
  - A first write with no stored preferences returns HTTP `200` and includes
    `"revision": 1`.
  - A subsequent write with `expectedRevision: 1` returns HTTP `200` and
    includes `"revision": 2`.
  - A stale write returns HTTP `409` with `code: "conflict"` and details
    containing `code: "revision_mismatch"`, `expectedRevision`, and
    `actualRevision`.
  - Omitting `expectedRevision` once the row exists returns HTTP `409`, not
    silent overwrite.
- Persistence behaviour:
  - the shared `user_preferences.revision` field increments exactly once per
    successful interests update;
  - `safety_toggle_ids` and `unit_system` survive an interests-only update;
  - DB-backed insert races produce deterministic conflict mapping rather than
    internal errors.
- Tests:
  - new `rstest` unit coverage passes;
  - new `rstest-bdd` behaviour coverage passes with embedded PostgreSQL;
  - `make test` passes.
- Lint/format/docs:
  - `make fmt`, `make markdownlint`, and `make nixie` pass after doc changes;
  - `make check-fmt` and `make lint` pass.
- Documentation:
  - `docs/wildside-backend-architecture.md` records the interests concurrency
    decision;
  - if payload shapes changed, `docs/wildside-pwa-data-model.md` matches them;
  - `docs/backend-roadmap.md` marks 3.5.4 done only after every gate above is
    green.

## Idempotence and recovery

This plan is intentionally re-runnable.

- Re-running focused tests is safe and expected.
- Re-running the embedded PostgreSQL suites is safe; they provision temporary
  databases and clean them up automatically.
- If `pg-embedded-setup-unpriv` fails with
  `cannot create /dev/null: Permission denied`, repair `/dev/null` to the
  standard character device before retrying and record that repair in the
  execution notes.
- If `make lint` fails because `yamllint` or `actionlint` is missing, install
  the required tool and rerun the same command; do not skip the lint stage.
- Do not mark the roadmap item complete until the final gate logs exist and
  show success.

## Artifacts and notes

Retain at least these logs:

- `/tmp/3-5-4-baseline-users-tests.out`
- `/tmp/3-5-4-red-unit.out`
- `/tmp/3-5-4-red-bdd.out`
- `/tmp/3-5-4-green-unit.out`
- `/tmp/3-5-4-green-bdd.out`
- `/tmp/3-5-4-fmt.out`
- `/tmp/3-5-4-markdownlint.out`
- `/tmp/3-5-4-nixie.out`
- `/tmp/3-5-4-check-fmt.out`
- `/tmp/3-5-4-lint.out`
- `/tmp/3-5-4-test.out`

Important evidence to capture in the final version of this plan:

- one passing transcript showing a stale-write scenario returns HTTP `409`;
- one passing transcript showing the success payload includes the incremented
  revision;
- one passing transcript showing full gates succeeded.

## Interfaces and dependencies

The implementation should end with these stable interfaces and relationships.

Recommended domain contract:

```rust
pub struct UpdateUserInterestsRequest {
    pub user_id: UserId,
    pub interest_theme_ids: Vec<InterestThemeId>,
    pub expected_revision: Option<u32>,
}

#[async_trait]
pub trait UserInterestsCommand: Send + Sync {
    async fn update(
        &self,
        request: UpdateUserInterestsRequest,
    ) -> Result<UserInterests, Error>;
}
```

Recommended interests model:

```rust
pub struct UserInterests {
    user_id: UserId,
    interest_theme_ids: Vec<InterestThemeId>,
    revision: u32,
}
```

Required adapter dependency direction:

- `backend::inbound::http::users::update_interests` depends on
  `backend::domain::ports::UserInterestsCommand`.
- `backend::outbound::persistence::DieselUserInterestsCommand` depends on
  `backend::domain::ports::UserPreferencesRepository`.
- `backend::domain` does not depend on `backend::outbound` or Actix types.

Required reuse of existing persistence contract:

- `backend::domain::ports::UserPreferencesRepository::find_by_user_id`
- `backend::domain::ports::UserPreferencesRepository::save`
- `backend::domain::ports::UserPreferencesRepositoryError`

Error mapping target:

- revision mismatches and missing-for-update paths map to `Error::conflict(...)`
  with details:
  - `code: "revision_mismatch"`
  - `expectedRevision: <number or null>`
  - `actualRevision: <number>`

This mirrors the existing preferences and annotations contracts and keeps the
interests endpoint aligned with the rest of the revisioned write surface.

## Revision note

Initial draft created on 2026-03-13 to prepare roadmap item 3.5.4 for
implementation. The draft locks the recommended aggregate and concurrency
strategy up front so implementation can proceed only after explicit user
approval and without re-opening the core architectural question.
