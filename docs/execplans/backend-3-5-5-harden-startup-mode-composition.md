# Harden startup-mode composition for user-state ports (roadmap 3.5.5)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: IN PROGRESS (implementation approved on 2026-05-26)

This plan covers roadmap item 3.5.5 only. Implementation began after explicit
approval on 2026-05-26.

## Purpose / big picture

Roadmap item 3.5.5 requires hardening
`backend/src/server/state_builders.rs`, the backend composition root that
chooses concrete HTTP-facing port implementations during startup. The current
selection rule is simple: when `ServerConfig.db_pool` is present, HTTP state
should use DB-backed adapters; when it is absent, HTTP state should use fixture
fallbacks. As more user-state wiring is added, that rule must remain
deterministic and visible in tests.

After the implementation, a developer can inspect one explicit user-state
composition helper in `backend/src/server/state_builders.rs` and see how
login, users, profile, interests, preferences, and route-annotation ports are
selected. Running the startup-mode unit and behavioural suites will prove that
DB-present mode does not silently fall back to fixtures, and fixture-fallback
mode still preserves the existing local contracts.

Observable success means:

- `backend/src/server/state_builders.rs` has explicit helper seams for
  user-state composition rather than separate ad hoc port selection calls.
- Existing public HTTP state construction remains source-compatible.
- DB-present startup mode uses DB-backed user-state adapters for every
  user-state port included in this item.
- Fixture-fallback startup mode uses deterministic fixture implementations
  when no DB pool is configured.
- Regression assertions fail before or during the helper-seam work if a
  DB-present path accidentally serves fixture identity data.
- `rstest` unit tests and `rstest-bdd` behavioural tests cover happy paths,
  unhappy DB-schema-loss paths, and mode-stability edge cases.
- `make check-fmt`, `make lint`, and `make test` pass with captured `/tmp`
  logs before any implementation commit is considered complete.

## Constraints

- Do not implement this plan until the user approves it.
- Scope is roadmap item 3.5.5 only. Do not complete roadmap item 3.5.6, which
  is reserved for the broader full startup matrix and revision-conflict
  expansion.
- Preserve hexagonal architecture invariants from the
  `$hexagonal-architecture` skill and `docs/wildside-backend-architecture.md`:
  domain code owns ports, outbound adapters implement driven ports, inbound
  handlers consume injected ports, and the composition root wires concrete
  adapters without moving persistence details into handlers or domain modules.
- Keep persistence details confined to outbound adapters. The plan may adjust
  startup wiring in `backend/src/server/state_builders.rs`, but it must not add
  Diesel queries, schema handling, or table-specific logic to inbound HTTP
  handlers.
- Preserve the current public shape of `build_http_state` unless a later
  approved plan revision explicitly allows an interface change.
- Do not add new external dependencies. Public Firecrawl research found the
  existing local stack sufficient: `rstest` for fixture and table-driven tests,
  `rstest-bdd` for behaviour scenarios, and `proptest` only if the
  implementation introduces a true input-space invariant.
- Use `leta` for symbol navigation and refactoring. Use text search only for
  documentation, configuration, literals, and feature files.
- Use `rstest` fixtures for new unit/regression tests. Use `rstest-bdd` for
  externally observable startup behaviour, following the scenario-binding and
  fixture conventions in `docs/developers-guide.md` and
  `docs/rstest-bdd-users-guide.md`.
- Do not run format, lint, or tests in parallel. Capture long command output
  with `tee` into `/tmp` logs.
- Commit after each coherent implementation milestone, but only after the
  milestone gates pass.
- Run `coderabbit review --agent` after each major milestone and clear all
  concerns before moving to the next milestone.
- If `docs/users-guide.md` remains absent, record that no user-guide update was
  possible and update the nearest relevant operator-facing document only if
  behaviour changes are visible to operators of the Wildside server
  application.

## Tolerances (exception triggers)

- Scope tolerance: if implementation needs changes outside
  `backend/src/server/state_builders.rs`, the startup-mode BDD suite,
  user-state startup tests, architecture/developer docs, and roadmap closure
  notes, stop and document the extra scope before proceeding.
- Churn tolerance: if the implementation exceeds 10 files or 600 net lines,
  stop and split the work into a smaller follow-up plan.
- Interface tolerance: if a public API signature, route contract, domain port
  signature, or persisted schema must change, stop and request approval.
- Dependency tolerance: if a new crate, service, or tool dependency seems
  necessary, stop and present alternatives.
- Behaviour tolerance: if DB-present mode cannot be made to hard-fail rather
  than fallback when a required user-state schema is missing, stop and document
  the conflict.
- Validation tolerance: if any required gate fails after three consecutive fix
  attempts, stop with the current logs and a root-cause summary.
- Architecture tolerance: if satisfying a test requires an inbound module to
  import outbound persistence code directly, stop and redesign the seam.
- Agent-team tolerance: sub-agents may perform reconnaissance only unless a
  later approved implementation pass assigns disjoint write ownership.

## Risks

- Risk: `HttpStateExtraPorts::default()` is fixture-first and could mask
  missing explicit composition if `build_http_state` stops wiring a port.
  Severity: medium.
  Likelihood: medium.
  Mitigation: keep `build_http_state` explicitly constructing all user-state
  and extra ports, and add assertions that inspect endpoint behaviour rather
  than relying only on type construction.

- Risk: the `Option<DbPool>` branch is repeated across helper functions, so a
  future port addition could accidentally use a fixture in DB-present mode.
  Severity: high.
  Likelihood: medium.
  Mitigation: introduce a user-state helper seam that groups related port
  selection in one place and add regression tests that prove DB identity data
  is returned in DB-present mode.

- Risk: route submission is composed in `backend/src/server/mod.rs`, not in
  `backend/src/server/state_builders.rs`, so route-submission parity can drift
  from the HTTP state matrix.
  Severity: medium.
  Likelihood: medium.
  Mitigation: keep this plan focused on user-state composition, but preserve
  existing route-submission assertions in the startup-mode BDD suite and avoid
  changing `build_route_submission_service` unless required by a failed test.

- Risk: embedded PostgreSQL tests can be skipped or flaky when local database
  setup fails.
  Severity: medium.
  Likelihood: medium.
  Mitigation: use existing `pg-embedded-setup-unpriv` helpers, preserve
  explicit skip diagnostics, and rely on the full `make test` gate before
  closure.

- Risk: over-hardening could duplicate the broader matrix planned for 3.5.6.
  Severity: medium.
  Likelihood: medium.
  Mitigation: limit new assertions to deterministic adapter selection and
  obvious no-fallback regressions for user-state wiring. Leave expanded
  revision-conflict and full-matrix repository coverage to 3.5.6.

## Agent team

Planning used a Wyvern agent team for read-only reconnaissance. During the
approved implementation pass, agent use remains optional and constrained:

- Coordinator: owns final design decisions, file edits, gates, commits,
  CodeRabbit review, push, and PR updates.
- Wyvern A, if reused: read-only documentation and roadmap verification.
- Wyvern B, if reused: read-only code and test reconnaissance.

Sub-agents must not run tests. If any worker-style implementation agents are
introduced after approval, each must receive a disjoint write scope and must be
told that other agents may be editing the repository.

## Progress

- [x] (2026-05-21) Loaded the requested `$leta`, `$rust-router`, and
  `$hexagonal-architecture` skills.
- [x] (2026-05-21) Created the Leta workspace for this worktree with
  `leta workspace add`.
- [x] (2026-05-21) Renamed the local branch to
  `backend-3-5-5-harden-startup-mode-composition`.
- [x] (2026-05-21) Used a Wyvern agent team for documentation and code
  reconnaissance.
- [x] (2026-05-21) Used Firecrawl to check current public references for
  `rstest`, `rstest-bdd`, `proptest`, and Rust ports-and-adapters prior art.
- [x] (2026-05-21) Drafted this pre-implementation ExecPlan.
- [x] (2026-05-26) Clarified that `docs/users-guide.md` references server
      operators, not end users of the product experience.
- [x] (2026-05-26) Obtained explicit user approval for this plan.
- [x] (2026-05-26) Ran Stage A baseline gates before implementation:
  `make check-fmt`, `make lint`, and `make test` all passed. The full test
  gate reported 1220 Rust tests passed with 4 skipped, followed by passing
  frontend/workspace tests.
- [x] (2026-05-26) Implemented the private `UserStatePortsBundle` and
  `compose_user_state_ports` seam in `backend/src/server/state_builders.rs`
  without changing the public `build_http_state` signature.
- [x] (2026-05-26) Ran Stage B targeted checks:
  `cargo test -p backend startup_modes_reject_invalid_credentials_with_unauthorised_envelope -- --nocapture`
  passed 2 selected tests, and
  `cargo test -p backend --test state_builders_composition_unit -- --nocapture`
  passed the fixture-mode composition unit test.
- [x] (2026-05-26) Ran Stage B milestone gates before CodeRabbit:
  `make check-fmt`, `make lint`, and `make test` all passed. The full test
  gate again reported 1220 Rust tests passed with 4 skipped, followed by
  passing frontend/workspace tests.
- [x] (2026-05-26) Ran CodeRabbit after Stage B helper-seam commit
  `af4d5d6`; the agent review completed with 0 findings.
- [x] (2026-05-26) Strengthened startup-mode behavioural regression
  assertions with a shared user-state adapter-selection helper that verifies
  current-user, users-list, and preferences evidence at the HTTP boundary.
- [x] (2026-05-26) Ran Stage C targeted BDD checks. The drafted filtered
  command selected 0 generated scenarios, so
  `cargo test -p backend --test startup_mode_composition_bdd -- --nocapture`
  was run instead and passed all 12 tests.
- [x] (2026-05-26) Ran Stage C milestone gates before CodeRabbit:
  `make check-fmt`, `make lint`, and `make test`.
  All passed. The full test gate reported 1220 Rust tests passed with 4
  skipped, followed by passing frontend/workspace tests.
- [x] (2026-05-26) Committed Stage C as `ce7e588` and ran
  `coderabbit review --agent`; the agent review completed with 0 findings.
- [x] (2026-05-26) Updated architecture, developer, and roadmap documentation
  after implementation evidence existed. No `docs/users-guide.md` update was
  made because this item preserves operator-visible server behaviour and the
  repository has no existing `docs/users-guide.md` file.
- [x] (2026-05-26) Re-ran final quality gates after documentation updates:
  `make check-fmt`, `make lint`, and `make test`.
  All passed. The full test gate reported 1220 Rust tests passed with 4
  skipped, followed by passing frontend/workspace tests.
- [ ] Run final CodeRabbit review after the documentation milestone and clear
  all concerns.
- [ ] Push the branch and update the draft PR with the final implementation
  summary and validation evidence.

## Surprises & discoveries

- Observation: `docs/users-guide.md` is not present in this checkout.
  Evidence: `fd 'users.*guide|user.*guide' docs` found specific tool guides
  but no repository-wide `docs/users-guide.md`.
  Impact: this plan records that user-guide updates are conditional. If
  implementation changes server behaviour that an operator of the Wildside
  server application should know about, update the relevant existing
  operator-facing document or create a separate documentation decision.

- Observation: `backend/tests/features/startup_mode_composition.feature`
  already states that it covers all HTTP-facing ports for roadmap item 3.5.5.
  Evidence: the feature file names all 16 `HttpStatePorts` and
  `HttpStateExtraPorts` and includes fixture, DB-present, schema-loss, and
  validation-stability scenarios.
  Impact: implementation should strengthen the existing suite rather than
  replace it wholesale.

- Observation: `build_http_state` is already the narrow composition point for
  HTTP state, while route-submission service construction lives in
  `backend/src/server/mod.rs`.
  Evidence: Leta found `state_builders.rs:build_http_state` and an ambiguous
  separate `mod.rs:build_route_submission_service`.
  Impact: this item should avoid moving route submission unless a regression
  test proves the current split prevents deterministic user-state composition.

- Observation: the Stage B targeted command named in the draft plan selects
  existing invalid-credential adapter tests rather than the
  `state_builders_composition_unit` test.
  Evidence:
  `cargo test -p backend startup_modes_reject_invalid_credentials_with_unauthorised_envelope -- --nocapture`
  passed two selected tests from `diesel_login_users_adapters.rs` and filtered
  out the state-builder composition test.
  Impact: keep the drafted command as historical evidence, but also run
  `cargo test -p backend --test state_builders_composition_unit -- --nocapture`
  for the helper-seam milestone.

- Observation: filtering the startup-mode BDD binary with
  `startup_mode_composition` selected support tests only and filtered out all
  generated scenarios.
  Evidence:
  `cargo test -p backend startup_mode_composition --test startup_mode_composition_bdd -- --nocapture`
  reported 0 executed tests and 12 filtered out.
  Impact: use
  `cargo test -p backend --test startup_mode_composition_bdd -- --nocapture`
  for this suite when validating generated `rstest-bdd` scenarios.

## Decision log

- Decision: keep this PR as a pre-implementation planning PR.
  Rationale: the user explicitly required plan approval before implementation.
  Date/Author: 2026-05-21 / Codex.

- Decision: group login, users, profile, interests, preferences, and
  route-annotation startup selection under an explicit user-state helper seam.
  Rationale: these ports share identity-bearing user-state semantics and are
  the highest-risk area for accidental DB-present fixture fallback.
  Date/Author: 2026-05-21 / Codex.

- Decision: do not introduce `proptest`, `kani`, or `verus` for the planned
  implementation unless the approved implementation introduces a true state
  transition invariant.
  Rationale: this hardening primarily concerns deterministic branch selection
  over two startup modes, which is better captured by table-driven `rstest`
  and behaviour-level `rstest-bdd` assertions.
  Date/Author: 2026-05-21 / Codex.

- Decision: assert users-list adapter selection by display-name evidence
  rather than by authenticated user ID in the shared startup-mode helper.
  Rationale: fixture fallback exposes the current user profile and users-list
  fixture as separate fixture records, while DB-present mode exposes the seeded
  database display name through both endpoints. Preferences still assert the
  authenticated fixture UUID to keep identity wiring covered.
  Date/Author: 2026-05-26 / Codex.

- Decision: document Firecrawl findings as supporting context, not as
  authoritative design input.
  Rationale: repository docs and local conventions are more specific than
  general public prior art. Public references only confirmed that the existing
  tools and ports-as-traits approach remain appropriate.
  Date/Author: 2026-05-21 / Codex.

- Decision: interpret `docs/users-guide.md` as an operator guide for the
  Wildside server application.
  Rationale: the implementation plan needs to distinguish server operator
  behaviour from product end-user behaviour when deciding whether a user-guide
  update is required.
  Date/Author: 2026-05-26 / Codex, based on user clarification.

- Decision: keep `compose_user_state_ports` private.
  Rationale: `build_http_state` remains the public composition entrypoint, and
  integration tests can verify observable fixture/DB behaviour without
  widening helper visibility.
  Date/Author: 2026-05-26 / Codex.

## Outcomes & retrospective

This section is intentionally empty during draft. It must be updated after
approval, implementation, CodeRabbit review, gate execution, and roadmap
closure.

## Context and orientation

`backend/src/server/state_builders.rs` constructs the Actix `web::Data` value
that carries backend ports into HTTP handlers. It imports domain port traits,
fixture implementations, and outbound adapter constructors, then chooses
between DB-backed and fixture-backed implementations based on
`ServerConfig.db_pool`.

The key functions and types are:

- `backend/src/server/state_builders.rs::build_http_state`, which builds
  `HttpStatePorts` and `HttpStateExtraPorts` before calling
  `HttpState::new_with_extra`.
- `backend/src/server/state_builders.rs::build_login_users_pair`, which chooses
  `DieselLoginService` plus `DieselUsersQuery` in DB-present mode and
  `FixtureLoginService` plus `FixtureUsersQuery` in fixture-fallback mode.
- `backend/src/server/state_builders.rs::build_profile_interests_pair`, which
  chooses `DieselUserProfileQuery` plus `DieselUserInterestsCommand` in
  DB-present mode and fixture implementations otherwise.
- `backend/src/server/state_builders.rs::build_user_preferences_pair`, created
  by the existing `build_idempotent_pair!` macro.
- `backend/src/server/state_builders.rs::build_route_annotations_pair`, also
  created by the existing `build_idempotent_pair!` macro.
- `backend/src/inbound/http/state.rs::HttpState`, `HttpStatePorts`, and
  `HttpStateExtraPorts`, which define the injected HTTP state bundle.
- `backend/src/server/mod.rs::build_route_submission_service`, which composes
  route submission outside `state_builders.rs`.

The most relevant tests are:

- `backend/tests/startup_mode_composition_bdd.rs` and
  `backend/tests/features/startup_mode_composition.feature`, which exercise
  startup-mode behaviour through HTTP-facing flows.
- `backend/tests/startup_mode_composition_bdd/flow_support.rs`, which stores
  snapshots and shared assertions.
- `backend/tests/startup_mode_composition_bdd/flows.rs`, which drives the
  happy and validation-error request flows.
- `backend/tests/user_state_startup_modes_bdd.rs`, which covers login/users
  startup mode behaviour.
- `backend/tests/user_state_profile_interests_startup_modes_bdd.rs`, which
  covers profile/interests startup mode behaviour.
- `backend/tests/diesel_login_users_adapters.rs` and
  `backend/tests/diesel_profile_interests_adapters.rs`, which exercise
  adapter-specific DB-backed behaviour.

Supporting documentation to consult during implementation:

- `docs/backend-roadmap.md` for roadmap item 3.5.5 and closure style.
- `docs/wildside-backend-architecture.md` for hexagonal module boundaries,
  composition-root guidance, and design-decision placement.
- `docs/developers-guide.md` for `rstest-bdd` v0.5.0 scenario conventions and
  behaviour-test layout.
- `docs/rust-testing-with-rstest-fixtures.md` for fixture scope, composition,
  and `#[once]` caveats.
- `docs/rstest-bdd-users-guide.md` for `ScenarioState`, `Slot<T>`, fixture
  injection, and async-step guidance.
- `docs/rust-doctest-dry-guide.md` if public examples or doc comments are
  changed.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` for helper
  extraction discipline if `state_builders.rs` grows too complex.
- `docs/pg-embed-setup-unpriv-users-guide.md` for embedded PostgreSQL fixture
  behaviour.
- `docs/documentation-style-guide.md` for roadmap and ADR/documentation style.

## Plan of work

### Stage A: approval and baseline evidence

Wait for explicit approval of this ExecPlan. After approval, re-read the plan,
confirm `git status --short --branch`, and verify that the branch is still
`backend-3-5-5-harden-startup-mode-composition`.

Run targeted baseline commands with logs:

```bash
make check-fmt 2>&1 | tee /tmp/check-fmt-wildside-backend-3-5-5-harden-startup-mode-composition.out
make lint 2>&1 | tee /tmp/lint-wildside-backend-3-5-5-harden-startup-mode-composition.out
make test 2>&1 | tee /tmp/test-wildside-backend-3-5-5-harden-startup-mode-composition.out
```

If any baseline gate fails for reasons unrelated to this plan, stop and record
the failure in `Surprises & Discoveries`.

### Stage B: add the explicit user-state seam

In `backend/src/server/state_builders.rs`, introduce a private helper type or
function that composes the identity-bearing user-state ports together. The
preferred shape is a small private bundle, for example
`UserStatePortsBundle`, returned by a helper such as
`compose_user_state_ports(config: &ServerConfig)`.

The helper should own calls to:

- `build_login_users_pair(config)`;
- `build_profile_interests_pair(config)`;
- `build_user_preferences_pair(config)`;
- `build_route_annotations_pair(config)`.

Then update `build_http_state` to destructure the bundle and pass its fields
into `HttpStatePorts`. Keep catalogue, offline bundles, walk sessions,
enrichment provenance, and route submission on their current paths unless tests
or review reveal a direct reason to group them too.

The helper must be private unless tests require `pub(super)` visibility. If
visibility is widened, document why in the decision log.

Add or adjust `rstest` unit tests close to existing startup-mode unit coverage.
The unit tests should table-drive `db_present: bool` where practical and assert
that user-state branch selection remains complete. Do not assert concrete type
names through brittle reflection if endpoint-level assertions give stronger
evidence.

Run targeted Rust tests for the changed module or nearest suite, then commit
this stage if it passes:

```bash
cargo test -p backend startup_modes_reject_invalid_credentials_with_unauthorised_envelope -- --nocapture 2>&1 | tee /tmp/test-targeted-user-state-seam-backend-3-5-5-harden-startup-mode-composition.out
```

After the commit, run:

```bash
coderabbit review --agent
```

Clear every actionable concern before proceeding.

### Stage C: strengthen behavioural regression assertions

Update `backend/tests/startup_mode_composition_bdd.rs` and the helper modules
under `backend/tests/startup_mode_composition_bdd/` to add a shared assertion
that the mode evidence is complete for user-state endpoints.

The preferred helper name is `assert_user_state_adapter_selection`, placed in
`backend/tests/startup_mode_composition_bdd/flow_support.rs` if it is reused
across more than one step. It should verify both:

- fixture-fallback mode returns fixture identity/profile evidence; and
- DB-present mode returns seeded DB identity/profile evidence.

Strengthen the existing schema-loss scenario so that DB-present mode with the
`users` table missing returns a stable internal-error envelope and does not set
later fixture-backed snapshots. Keep this as a no-fallback assertion rather
than broadening into every 3.5.6 matrix case.

If the feature file needs clearer language, update
`backend/tests/features/startup_mode_composition.feature` without changing
scenario intent. Keep step names aligned with `rstest-bdd` bindings.

Run the relevant behavioural tests with logs:

```bash
cargo test -p backend startup_mode_composition --test startup_mode_composition_bdd -- --nocapture 2>&1 | tee /tmp/test-startup-mode-composition-bdd-backend-3-5-5-harden-startup-mode-composition.out
```

Commit this stage only after the targeted BDD suite passes or is skipped solely
by existing embedded PostgreSQL skip handling. Run `coderabbit review --agent`
and clear concerns before proceeding.

### Stage D: documentation and roadmap closure

Update documentation only after implementation evidence exists.

Update `docs/wildside-backend-architecture.md` with a short design-decision
entry that records the explicit user-state composition helper seam and why it
exists. If the change is more substantive than a local composition decision,
create an ADR following `docs/documentation-style-guide.md` and link it from
the architecture document.

Update `docs/developers-guide.md` if the implementation introduces a reusable
startup-mode test convention, helper naming convention, or BDD layout practice
that future contributors need to follow.

Update `docs/users-guide.md` only if that file exists or if a server behaviour
change is introduced that an operator of the Wildside server application should
know about. If it remains absent and operator-visible behaviour is unchanged,
record this in the plan rather than creating a generic guide solely for this
item.

Update `docs/backend-roadmap.md` to mark 3.5.5 done only after the helper seam,
tests, CodeRabbit review, and full gates succeed. Follow the style used by
3.5.2 and 3.5.3: include a short execution note naming the main tests and
gate evidence.

Run Markdown and repository gates:

```bash
make check-fmt 2>&1 | tee /tmp/check-fmt-wildside-backend-3-5-5-harden-startup-mode-composition.out
make lint 2>&1 | tee /tmp/lint-wildside-backend-3-5-5-harden-startup-mode-composition.out
make test 2>&1 | tee /tmp/test-wildside-backend-3-5-5-harden-startup-mode-composition.out
```

Commit documentation and roadmap closure only after the gates pass. Run
`coderabbit review --agent` one final time and clear concerns.

## Concrete steps

All commands run from:

```plaintext
/home/leynos/.lody/repos/github---leynos---wildside/worktrees/f407c9df-ba22-40a7-842e-5e0eb11778b9
```

Before implementation:

```bash
git status --short --branch
git branch --show-current
```

Expected branch:

```plaintext
backend-3-5-5-harden-startup-mode-composition
```

Use Leta for code navigation:

```bash
leta show state_builders.rs:build_http_state -n 20
leta show build_login_users_pair -n 10
leta show build_profile_interests_pair -n 10
leta grep "startup_mode" "backend/tests" -k function,method
```

Implement Stage B and Stage C with small patches. Before each commit:

```bash
git diff --check
git status --short
```

Commit with a file-based message:

```bash
COMMIT_MSG_DIR=$(mktemp -d)
cat > "$COMMIT_MSG_DIR/COMMIT_MSG.md" << 'ENDOFMSG'
Harden user-state startup composition

Group user-state HTTP port wiring behind an explicit composition helper
and add regression assertions that keep DB-present and fixture-fallback
startup modes deterministic.
ENDOFMSG
git commit -F "$COMMIT_MSG_DIR/COMMIT_MSG.md"
rm -rf "$COMMIT_MSG_DIR"
```

Use an appropriately narrower summary for each actual milestone commit.

## Validation and acceptance

Acceptance requires all of the following after approved implementation:

- `build_http_state` still returns `web::Data<HttpState>` and existing callers
  compile unchanged.
- User-state ports are visibly grouped through an explicit helper seam in
  `backend/src/server/state_builders.rs`.
- Fixture-fallback startup mode returns fixture identity/profile evidence for
  user-state endpoints.
- DB-present startup mode returns seeded DB identity/profile evidence for
  user-state endpoints.
- DB-present startup with the `users` table missing returns an internal-error
  envelope and does not continue with fixture-backed profile or preferences
  snapshots.
- Validation error envelopes remain stable across fixture-fallback and
  DB-present modes.
- `rstest` unit/regression coverage exists for deterministic selection or the
  nearest existing unit suite is expanded with a table-driven case.
- `rstest-bdd` behavioural coverage exists for HTTP-observable startup-mode
  behaviour.
- Architecture or developer documentation records any new internal convention.
- `docs/backend-roadmap.md` marks item 3.5.5 done only after all validation
  succeeds.
- `coderabbit review --agent` has no unresolved actionable concerns.
- Final gates pass:

```bash
make check-fmt
make lint
make test
```

Property tests, Kani, and Verus are not acceptance requirements for the draft
scope. Reconsider this decision only if implementation introduces a new
invariant over a range of input states, state transitions, or ordering
constraints that cannot be covered by the two startup modes and current BDD
matrix.

## Idempotence and recovery

The implementation steps are additive and can be repeated. If a targeted test
fails, inspect the corresponding `/tmp` log before changing code. If a
CodeRabbit review raises a concern that would exceed a tolerance, update this
plan and ask for approval before proceeding.

If an implementation commit proves wrong, prefer a new corrective commit over
rewriting shared history after the branch is pushed. If the branch has not been
pushed and no other agent depends on it, an amend is acceptable for fixing the
immediate previous commit after rerunning the relevant gate.

Embedded PostgreSQL setup failures should use the repository's existing skip
diagnostics. Do not replace them with silent fixture fallback.

## Artifacts and notes

Planning evidence collected before this draft:

- `leta workspace add` registered this worktree.
- `leta grep ".*" "backend/src/server/state_builders.rs" -k function,method,struct,enum`
  identified `build_http_state`, user-state pair builders, and extra port
  builders.
- Firecrawl searches found current public references for:
  - `https://docs.rs/rstest`;
  - `https://docs.rs/rstest-bdd`;
  - `https://docs.rs/proptest`.
- Wyvern documentation reconnaissance confirmed roadmap scope, documentation
  update targets, and `rstest-bdd` v0.5.0 conventions.
- Wyvern code reconnaissance identified the current composition seam, existing
  startup-mode BDD suite, and `HttpStateExtraPorts::default()` masking risk.

## Interfaces and dependencies

No new dependency is planned.

The planned private helper in `backend/src/server/state_builders.rs` should use
existing domain port trait objects:

```rust
struct UserStatePortsBundle {
    login: Arc<dyn LoginService>,
    users: Arc<dyn UsersQuery>,
    profile: Arc<dyn UserProfileQuery>,
    interests: Arc<dyn UserInterestsCommand>,
    preferences: Arc<dyn UserPreferencesCommand>,
    preferences_query: Arc<dyn UserPreferencesQuery>,
    route_annotations: Arc<dyn RouteAnnotationsCommand>,
    route_annotations_query: Arc<dyn RouteAnnotationsQuery>,
}

fn compose_user_state_ports(config: &ServerConfig) -> UserStatePortsBundle;
```

The exact names may change during implementation if a shorter local convention
fits better, but the helper must remain explicit and private unless tests need
`pub(super)` access.

## Revision note

Initial draft created on 2026-05-21. The draft captures roadmap item 3.5.5
scope, current code anchors, test strategy, Firecrawl research findings,
Wyvern reconnaissance, approval gate, and implementation tolerances. No
implementation has been performed under this plan.

Revision on 2026-05-26 clarifies that `docs/users-guide.md` means an operator
guide for the Wildside server application. This narrows user-guide update
requirements to operator-visible server behaviour and leaves implementation
scope otherwise unchanged.
