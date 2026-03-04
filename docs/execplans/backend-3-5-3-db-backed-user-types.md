# Replace fixture-backed profile and interests wiring with DB-backed adapters (roadmap 3.5.3)

This Execution Plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises &
Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must be kept up
to date as work proceeds.

Status: DRAFT

This plan covers roadmap item 3.5.3 only:
`Replace fixture-backed UserProfileQuery and UserInterestsCommand wiring with
explicit DB-backed concrete types, and document whether this uses repository
extensions or dedicated adapters.`

## Purpose / big picture

`backend/src/server/state_builders.rs` now uses DB-backed adapters for
`LoginService` and `UsersQuery` (roadmap 3.5.2), but it still hard-wires
`FixtureUserProfileQuery` and `FixtureUserInterestsCommand` even when
`ServerConfig.db_pool` is present.

After this change, DB-present startup will wire explicit DB-backed concrete
types for both `UserProfileQuery` and `UserInterestsCommand`, while DB-absent
startup will keep fixture fallback behaviour. The implementation must make the
adapter strategy explicit in architecture documentation: either repository
extensions (for example extending `DieselUserRepository`) or dedicated adapters
(for example `DieselUserProfileQuery` and `DieselUserInterestsCommand`).

Observable success criteria:

- DB-present startup path uses DB-backed profile/interests ports.
- DB-absent startup path still uses fixture profile/interests ports.
- `GET /api/v1/users/me` and `PUT /api/v1/users/me/interests` preserve session
  and error-envelope behaviour.
- Unit tests (`rstest`) and behavioural tests (`rstest-bdd`) cover happy,
  unhappy, and edge cases.
- Embedded PostgreSQL test flows run via `pg-embedded-setup-unpriv`.
- `docs/wildside-backend-architecture.md` records the adapter strategy decision
  for 3.5.3.
- `docs/backend-roadmap.md` marks only 3.5.3 as done after all gates pass.
- `make check-fmt`, `make lint`, and `make test` succeed with logs retained.

## Constraints

- Scope is roadmap item 3.5.3 only. Do not implement roadmap items 3.5.4,
  3.5.5, or 3.5.6 in this change.
- Preserve hexagonal boundaries:
  - domain owns port traits and domain errors;
  - outbound owns Diesel SQL and row mapping;
  - inbound handlers consume ports only.
- Preserve fixture fallback when `config.db_pool` is `None`.
- Keep endpoint contracts stable for:
  - `GET /api/v1/users/me`;
  - `PUT /api/v1/users/me/interests`.
- Do not add new migrations in 3.5.3.
- Do not add new external dependencies.
- Use `rstest` for unit and integration coverage and `rstest-bdd` for
  behavioural coverage.
- Use `pg-embedded-setup-unpriv` test support for DB-backed suites.
- Keep Markdown style consistent with repository docs standards.

## Tolerances (exception triggers)

- Scope tolerance: if implementation requires introducing the full
  revision-conflict contract for interests writes, stop and escalate because
  that is 3.5.4 scope.
- Interface tolerance: if public HTTP API signatures must change, stop and
  escalate.
- Churn tolerance: if the change exceeds 14 files or 1,100 net LOC, stop and
  split follow-up scope.
- Dependency tolerance: if a new crate would be required, stop and escalate.
- Test tolerance: if embedded PostgreSQL tests remain flaky after adding
  explicit skip handling and deterministic setup, stop and document options.
- Gate tolerance: if `make check-fmt`, `make lint`, or `make test` fails after
  three fix loops, stop and capture evidence.

## Risks

- Risk: interests persistence is still dual-model (`user_preferences` and
  `user_interest_themes`), which can cause ambiguous adapter ownership.
  Severity: high.
  Likelihood: high.
  Mitigation: decide and document canonical adapter strategy for 3.5.3, and
  explicitly defer revision-safe write contract details to 3.5.4.

- Risk: missing-user or missing-table behaviour could change envelope
  semantics.
  Severity: high.
  Likelihood: medium.
  Mitigation: add unhappy-path assertions for status, `code`, `message`, and
  trace-id parity.

- Risk: state-builder wiring may drift between DB-present and fixture modes.
  Severity: medium.
  Likelihood: medium.
  Mitigation: add dedicated startup-mode behavioural scenarios for
  profile/interests flows.

- Risk: adding profile/interests adapters by extending an existing repository
  may create low-cohesion abstractions.
  Severity: medium.
  Likelihood: medium.
  Mitigation: evaluate dedicated adapters first and record the final decision
  in architecture docs.

## Progress

- [x] (2026-03-04 13:50Z) Reviewed roadmap item 3.5.3 and adjacent completed
  item 3.5.2.
- [x] (2026-03-04 13:50Z) Captured current wiring and test seams from
  `state_builders`, ports, and startup-mode suites.
- [x] (2026-03-04 13:50Z) Drafted this ExecPlan at
  `docs/execplans/backend-3-5-3-db-backed-user-types.md`.
- [ ] Implement DB-backed profile/interests adapters and state wiring.
- [ ] Add/extend `rstest` and `rstest-bdd` coverage for happy/unhappy/edge
  paths.
- [ ] Record architecture decision and mark roadmap item 3.5.3 done.
- [ ] Run full gates (`make check-fmt`, `make lint`, `make test`) and capture
  evidence logs.

## Surprises & discoveries

- Observation: `build_http_state` still has an explicit TODO for profile and
  interests fixture wiring.
  Evidence: `backend/src/server/state_builders.rs`.
  Impact: 3.5.3 can stay focused on these two ports without broad refactoring.

- Observation: there are no existing outbound concrete types for
  `UserProfileQuery` or `UserInterestsCommand`.
  Evidence: `backend/src/outbound/persistence/mod.rs` exports none.
  Impact: 3.5.3 requires new concrete adapters or repository trait
  implementations.

- Observation: interests storage is currently dual-model in schema.
  Evidence: `docs/user-state-schema-audit-3-5-1.md` and
  `backend/src/outbound/persistence/schema.rs`.
  Impact: adapter strategy must be documented clearly to avoid hidden coupling.

## Decision Log

- Decision: keep this ExecPlan in `DRAFT` until implementation is requested and
  explicitly approved.
  Rationale: `execplans` workflow separates drafting from execution.
  Date/Author: 2026-03-04 / Codex.

- Decision: plan assumes dedicated adapters are the preferred default
  (`DieselUserProfileQuery`, `DieselUserInterestsCommand`) and requires an
  explicit architecture note if repository extension is chosen instead.
  Rationale: aligns with 3.5.2 adapter-wrapping pattern and keeps boundaries
  explicit.
  Date/Author: 2026-03-04 / Codex.

## Outcomes & retrospective

Pending implementation.

Completion notes must include:

- chosen adapter strategy and rationale;
- test artefacts added for 3.5.3;
- gate evidence log paths;
- any deferred follow-up work for 3.5.4+.

## Context and orientation

Primary references:

- `docs/backend-roadmap.md` (3.5.3 requirement and closure checkbox).
- `docs/wildside-backend-architecture.md` (hexagonal boundaries and decision
  log section).
- `docs/user-state-schema-audit-3-5-1.md` (schema coverage constraints).
- `docs/rust-testing-with-rstest-fixtures.md`.
- `docs/rstest-bdd-users-guide.md`.
- `docs/pg-embed-setup-unpriv-users-guide.md`.
- `docs/rust-doctest-dry-guide.md`.
- `docs/complexity-antipatterns-and-refactoring-strategies.md`.

Current code anchors:

- `backend/src/server/state_builders.rs`.
- `backend/src/domain/ports/user_profile_query.rs`.
- `backend/src/domain/ports/user_interests_command.rs`.
- `backend/src/inbound/http/users.rs`.
- `backend/src/outbound/persistence/diesel_user_repository.rs`.
- `backend/src/outbound/persistence/diesel_user_preferences_repository.rs`.
- `backend/src/outbound/persistence/mod.rs`.
- `backend/tests/diesel_login_users_adapters.rs`.
- `backend/tests/user_state_startup_modes_bdd.rs`.
- `backend/tests/support/embedded_postgres.rs`.

## Plan of work

Stage A: lock strategy and write failing tests first.

Decide whether 3.5.3 uses repository extensions or dedicated adapters. The
default path is dedicated adapters. Before implementing wiring, add or extend
test coverage so DB-backed profile/interests expectations fail in red state.
This stage ends only when failing tests clearly demonstrate missing behaviour.

Stage B: add outbound concrete types and state-builder wiring.

Implement explicit DB-backed concrete types for `UserProfileQuery` and
`UserInterestsCommand` under `backend/src/outbound/persistence/`, export them
from `mod.rs`, and wire them in `build_http_state` only when `db_pool` is
present. Keep fixture fallback unchanged for DB-absent mode.

Stage C: expand behavioural coverage and error-path parity.

Add startup-mode behavioural tests for profile/interests using
`rstest-bdd` and embedded PostgreSQL helpers, covering happy mode selection,
unhappy schema-loss behaviour, and one edge case that proves validation/session
contracts remain stable.

Stage D: documentation, roadmap closure, and gate replay.

Record the final adapter strategy decision in
`docs/wildside-backend-architecture.md`, mark roadmap item 3.5.3 done in
`docs/backend-roadmap.md`, then run full repository gates and retain logs.

## Concrete steps

1. Baseline and red-state setup.

Run existing startup-mode suites to capture baseline, then add new/extended
tests for profile/interests that fail before implementation.

```bash
set -o pipefail
cargo nextest run -p backend --test diesel_login_users_adapters --no-fail-fast \
  2>&1 | tee /tmp/3-5-3-baseline-login-users.out

set -o pipefail
cargo nextest run -p backend --test user_state_startup_modes_bdd --no-fail-fast \
  2>&1 | tee /tmp/3-5-3-baseline-startup-bdd.out
```

Expected red evidence after adding new tests:

```plaintext
New profile/interests DB-present expectations fail while fixture wiring remains.
```

2. Implement adapters and DB-present wiring.

Create concrete types (preferred names):

- `backend/src/outbound/persistence/diesel_user_profile_query.rs`.
- `backend/src/outbound/persistence/diesel_user_interests_command.rs`.

Update:

- `backend/src/outbound/persistence/mod.rs` exports.
- `backend/src/server/state_builders.rs` DB-present profile/interests wiring.

3. Add integration and behavioural suites.

Add or extend:

- `backend/tests/diesel_profile_interests_adapters.rs` (`rstest`) for
  DB-present and fixture-fallback startup-mode adapter outcomes.
- `backend/tests/features/user_state_profile_interests_startup_modes.feature`
  (`rstest-bdd`) with happy/unhappy/edge scenarios.
- `backend/tests/user_state_profile_interests_startup_modes_bdd.rs`
  step bindings and assertions.

Use `backend/tests/support/embedded_postgres.rs` helpers and
`handle_cluster_setup_failure` skip semantics.

4. Run targeted suites before full gates.

```bash
set -o pipefail
cargo nextest run -p backend --test diesel_profile_interests_adapters --no-fail-fast \
  2>&1 | tee /tmp/3-5-3-profile-interests-rstest.out

set -o pipefail
cargo nextest run -p backend --test user_state_profile_interests_startup_modes_bdd --no-fail-fast \
  2>&1 | tee /tmp/3-5-3-profile-interests-bdd.out
```

5. Documentation and closure.

Update architecture design decisions to state the chosen adapter strategy and
why. Then mark roadmap 3.5.3 `[x]` only.

6. Final quality gates.

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/3-5-3-check-fmt.out

set -o pipefail
make lint 2>&1 | tee /tmp/3-5-3-lint.out

set -o pipefail
make test 2>&1 | tee /tmp/3-5-3-test.out
```

## Validation and acceptance

Acceptance is behavioural, not structural.

- Fixture-fallback mode:
  - `GET /api/v1/users/me` still returns fixture-shaped profile.
  - `PUT /api/v1/users/me/interests` still behaves as fixture fallback.
- DB-present mode:
  - profile and interests paths use DB-backed concrete adapters.
  - responses remain contract-compatible for success and error envelopes.
- Unhappy coverage:
  - invalid credentials/session paths remain stable;
  - DB schema-loss scenarios produce stable error envelopes.
- Edge coverage:
  - interests request validation (`interestThemeIds` constraints) stays stable
    under DB-present mode.
- All required gates pass:
  - `make check-fmt`;
  - `make lint`;
  - `make test`.

## Idempotence and recovery

- All test commands are safe to rerun.
- Embedded PostgreSQL setup is isolated per test database and uses shared
  template provisioning helpers.
- If a suite fails mid-run:
  - fix the failing scope;
  - rerun the targeted suite first;
  - rerun final full gates before closure.
- If DB bootstrap is unavailable, suites should skip with
  `SKIP-TEST-CLUSTER` messaging rather than false-failing unrelated work.

## Artifacts and notes

Capture and retain:

- targeted test logs under `/tmp/3-5-3-*.out`;
- final gate logs:
  - `/tmp/3-5-3-check-fmt.out`,
  - `/tmp/3-5-3-lint.out`,
  - `/tmp/3-5-3-test.out`;
- final file list and rationale in the completion update.

## Interfaces and dependencies

Expected interfaces at completion:

```rust
pub struct DieselUserProfileQuery { /* adapter state */ }

#[async_trait]
impl UserProfileQuery for DieselUserProfileQuery {
    async fn fetch_profile(&self, user_id: &UserId) -> Result<User, Error>;
}

pub struct DieselUserInterestsCommand { /* adapter state */ }

#[async_trait]
impl UserInterestsCommand for DieselUserInterestsCommand {
    async fn set_interests(
        &self,
        user_id: &UserId,
        interest_theme_ids: Vec<InterestThemeId>,
    ) -> Result<UserInterests, Error>;
}
```

State-builder seam to be updated:

```rust
fn build_profile_interests_pair(
    config: &ServerConfig,
) -> (
    Arc<dyn UserProfileQuery>,
    Arc<dyn UserInterestsCommand>,
);
```

No new dependencies are expected. Reuse existing Diesel adapters, domain ports,
and `pg-embedded-setup-unpriv` test helpers.

## Revision note

Initial draft created for roadmap 3.5.3 planning. The draft codifies current
wiring gaps, proposes a dedicated-adapter default, and defines explicit
validation and closure gates.
