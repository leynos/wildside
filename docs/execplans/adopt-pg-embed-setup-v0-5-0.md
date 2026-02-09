# Migrate embedded Postgres tests to pg-embed-setup-unpriv v0.5.0

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

No `PLANS.md` file exists in this repository root.

## Purpose / Big Picture

Upgrade backend test infrastructure from `pg-embed-setup-unpriv` `0.4.0` to
`0.5.0` and adopt the new lifecycle and test-focused functionality where it
materially improves this repository's test suite. After this work, contributors
can run the same test suites with more predictable cluster lifecycle behaviour,
clearer backend selection semantics, and safer shared-cluster patterns for
future send-bound tests.

Success is observable when:

- `backend/Cargo.toml` depends on `pg-embed-setup-unpriv` `0.5.0`.
- Embedded Postgres tests continue to pass using the shared-template strategy.
- The migration adopts v0.5.0 capabilities relevant to this suite, especially
  strict `PG_TEST_BACKEND` handling and handle/guard split APIs for shared
  cluster access.
- `docs/developers-guide.md` describes the resulting strategy coherently.
- `make check-fmt`, `make lint`, and `make test` all succeed.

## Constraints

- Keep production runtime behaviour unchanged; this migration is test
  infrastructure and contributor documentation only.
- Preserve the current test isolation model: one shared cluster per test
  process with per-test temporary databases cloned from a migration template.
- Do not weaken skip/failure semantics in cluster setup. `SKIP-TEST-CLUSTER`
  must remain explicit and controlled.
- Do not add new third-party dependencies.
- Keep Rust and Markdown changes compliant with repository quality gates.
- Keep documentation in en-GB-oxendict spelling and wrapped to repository
  formatting rules.

If any objective requires violating a constraint, stop and escalate.

## Tolerances (Exception Triggers)

- Scope: if implementation requires touching more than 12 tracked files or
  exceeds 500 net changed lines, stop and escalate before proceeding.
- Interface: if a non-test public API in `backend/src/` must change, stop and
  escalate.
- Dependency graph: if migration requires additional crates, features, or
  toolchain changes beyond the version bump to `pg-embed-setup-unpriv` `0.5.0`,
  stop and escalate.
- Iterations: if `make test` fails after 3 focused repair attempts, stop and
  escalate.
- Ambiguity: if the best adoption path between `shared_test_cluster_handle()`
  and custom `new_split()` wiring is unclear after code inspection, stop and
  present options with trade-offs.

## Risks

- Risk: strict `PG_TEST_BACKEND` validation in v0.5.0 could make previously
  permissive local or CI configurations fail early.
  Severity: high
  Likelihood: medium
  Mitigation: audit CI and test helper environment assumptions, set
  `PG_TEST_BACKEND=postgresql_embedded` where required, and add explicit
  documentation.

- Risk: moving shared test helpers from `TestCluster` references to
  `ClusterHandle`/guard split could create lifecycle leaks or cleanup drift.
  Severity: medium
  Likelihood: medium
  Mitigation: keep process-lifetime ownership explicit, retain template-db
  cleanup guards (`TemporaryDatabase`), and validate teardown behaviour with
  focused tests.

- Risk: default cleanup behaviour changed in v0.5.0 (`DataOnly` on drop) and
  could surprise debugging workflows that inspect directories.
  Severity: medium
  Likelihood: medium
  Mitigation: document when to use `CleanupMode::None` for forensic runs and
  keep deterministic defaults for automated tests.

- Risk: changing support helpers may silently affect many integration suites.
  Severity: medium
  Likelihood: high
  Mitigation: migrate helper signatures incrementally, then run full gates.

## Progress

- [x] (2026-02-09 02:32Z) Drafted ExecPlan from repository context and
  v0.5.0 migration/user guides.
- [x] (2026-02-09 11:30Z) Approval received for implementation phase.
- [x] (2026-02-09 11:31Z) Upgraded dependency target in `backend/Cargo.toml`
  and started lockfile refresh.
- [x] (2026-02-09 11:34Z) Migrated shared cluster support to v0.5.0
  handle/guard split APIs.
- [x] (2026-02-09 11:35Z) Applied helper call-site type updates for
  `ClusterHandle`.
- [x] (2026-02-09 11:36Z) Updated contributor and workflow documentation for
  v0.5.0 backend-selection and fixture usage.
- [x] (2026-02-09 11:38Z) Ran `make check-fmt`, `make lint`, and `make test`
  successfully.
- [x] (2026-02-09 11:38Z) Committed migration and documentation changes.

## Surprises & Discoveries

- Observation: repository docs are inconsistent about the currently adopted
  pg-embed version (`docs/wildside-testing-guide.md` still states `v0.2.0`,
  while `backend/Cargo.toml` currently uses `0.4.0`).
  Evidence: direct file inspection.
  Impact: migration must include cohesive documentation alignment so strategy
  docs do not drift from code reality.

- Observation: current helper `backend/tests/support/pg_embed.rs` wraps
  `test_support::shared_cluster()` with environment locking and retry logic,
  so adoption of `shared_test_cluster_handle()` may require a custom wrapper
  rather than a direct fixture swap.
  Evidence: helper code inspection.
  Impact: plan keeps both adoption options open until implementation chooses
  the least risky path.

- Observation: repository-level `make test` includes additional package suites
  beyond backend integration tests, so quality-gate verification must allow for
  significantly longer runtime than backend-only nextest.
  Evidence: `make test` logs include `mxd` and `mxd-verification` suites after
  workspace nextest.
  Impact: implementation validation used full gate logs with `tee` and waited
  for final command completion markers.

## Decision Log

- Decision: create a dedicated ExecPlan at
  `docs/execplans/adopt-pg-embed-setup-v0-5-0.md` before implementing changes.
  Rationale: user requested explicit planning with execplans skill and this
  migration has non-trivial behavioural changes.
  Date/Author: 2026-02-09 / Codex.

- Decision: treat documentation cohesion as part of migration scope,
  specifically updating `docs/developers-guide.md` to reflect test strategy
  and v0.5.0 usage guidance.
  Rationale: user requirement and existing documentation drift.
  Date/Author: 2026-02-09 / Codex.

- Decision: implement shared test bootstrap with `TestCluster::new_split()`,
  retain process-lifetime ownership via intentional `ClusterGuard` leak, and
  expose only `&'static ClusterHandle` to tests.
  Rationale: this adopts v0.5.0 send-safe lifecycle APIs while preserving the
  existing sandbox path overrides and retry controls in local helpers.
  Date/Author: 2026-02-09 / Codex.

- Decision: set `PG_TEST_BACKEND=postgresql_embedded` explicitly in both CI
  test environment and local shared-cluster bootstrap when unset.
  Rationale: v0.5.0 validates backend values strictly; explicit defaulting keeps
  bootstrap deterministic and aligned between local and CI runs.
  Date/Author: 2026-02-09 / Codex.

## Outcomes & Retrospective

Migration implementation completed within plan tolerances. Backend test helpers
now run on `pg-embed-setup-unpriv` `0.5.0`, shared cluster ownership uses
`ClusterHandle` from the split lifecycle APIs, and CI/local guidance now
documents strict backend selection for `PG_TEST_BACKEND`.

Validation outcome:

- `make check-fmt` passed.
- `make lint` passed.
- `make test` passed.

No plan tolerances were exceeded.

## Context and Orientation

This repository already uses shared embedded-Postgres infrastructure for backend
integration and behavioural tests.

Key files and their current role:

- `backend/Cargo.toml`: currently pins `pg-embed-setup-unpriv` at `0.4.0`.
- `backend/tests/support/pg_embed.rs`: shared cluster bootstrap wrapper with
  environment locking and retry handling.
- `backend/tests/support/embedded_postgres.rs`: template-database helper using
  `temporary_database_from_template` and migration-hash template naming.
- `backend/tests/*` suites (for example `diesel_user_repository.rs`,
  `ports_behaviour.rs`, `schema_baseline_bdd.rs`): consume the shared helper.
- `backend/tests/pg_embedded_smoke.rs`: opt-in smoke test that still uses a
  direct `TestCluster::new()` constructor.
- `.github/workflows/ci.yml`: test workflow currently documents v0.4.0 worker
  requirements and does not explicitly set `PG_TEST_BACKEND`.
- `docs/developers-guide.md`: canonical contributor workflow and strategy guide
  that must reflect final migration usage.

Terms used in this plan:

- `ClusterHandle`: send-safe cluster access in v0.5.0.
- `ClusterGuard`: lifecycle owner that shuts down cluster and restores
  environment on drop.
- Shared cluster strategy: one embedded cluster per test process, plus
  per-test temporary databases for isolation.
- Template database: pre-migrated database cloned quickly for each test.

## Plan of Work

### Stage A: Confirm migration surface and choose shared-cluster adoption path

Inspect test helper code and choose whether to adopt v0.5.0 shared access using
`shared_test_cluster_handle()` directly or a custom wrapper built on
`TestCluster::new_split()` that preserves existing environment locking/retries.

Go/no-go validation:

- A documented choice exists in this ExecPlan `Decision Log`.
- The chosen path preserves current isolation and skip semantics.

### Stage B: Upgrade dependency and align helper APIs

Update `backend/Cargo.toml` and refresh `Cargo.lock` to `0.5.0`. Then migrate
shared helper interfaces in `backend/tests/support/pg_embed.rs` and
`backend/tests/support/embedded_postgres.rs` to use v0.5.0 lifecycle/test
capabilities selected in Stage A.

Expected adoption of new functionality:

- Handle/guard split for shared cluster ownership where appropriate.
- Strict `PG_TEST_BACKEND` compatibility handling and documentation.
- Explicit cleanup guidance using `CleanupMode` for debug workflows.

Go/no-go validation:

- Helper modules compile.
- Representative suites compile with migrated helper signatures.

### Stage C: Migrate call sites and CI contract

Apply necessary call-site updates across backend tests that consume shared
helpers. Update `.github/workflows/ci.yml` comments and environment contract as
needed for v0.5.0 semantics, especially `PG_TEST_BACKEND` handling.

Go/no-go validation:

- All embedded-Postgres test files compile.
- CI config text matches v0.5.0 behaviour and no stale version notes remain.

### Stage D: Documentation, hardening, and verification

Update `docs/developers-guide.md` so it coherently describes the embedded
Postgres strategy and v0.5.0 usage conventions adopted by the migration.

Run mandatory quality gates and capture logs.

Go/no-go validation:

- `make check-fmt`, `make lint`, and `make test` pass.
- Documentation and code references are coherent.

## Concrete Steps

Run all commands from repository root:
`/data/leynos/Projects/wildside.worktrees/adopt-pg-embed-setup-v0-5-0`.

1. Baseline and dependency upgrade:

    git branch --show
    rg -n "pg-embed-setup-unpriv|pg_embedded_setup_unpriv" backend/Cargo.toml backend/tests
    cargo update -p pg-embed-setup-unpriv --precise 0.5.0

2. Migrate helper code and test call sites as described in Stage B and C.

3. Verify documentation references:

    rg -n "pg-embed-setup-unpriv|PG_TEST_BACKEND|ClusterHandle|CleanupMode" docs/developers-guide.md docs/wildside-testing-guide.md .github/workflows/ci.yml

4. Run quality gates with `tee` logs:

    PROJECT="$(basename "$(git rev-parse --show-toplevel)")"
    BRANCH="$(git branch --show)"
    make check-fmt | tee "/tmp/check-fmt-${PROJECT}-${BRANCH}.out"
    make lint | tee "/tmp/lint-${PROJECT}-${BRANCH}.out"
    make test | tee "/tmp/test-${PROJECT}-${BRANCH}.out"

Expected success indicators:

- Each command exits with status `0`.
- Gate logs end without failures.

## Validation and Acceptance

Acceptance is behavioural and repository-visible:

- Dependency migration:
  `backend/Cargo.toml` and `Cargo.lock` resolve to `pg-embed-setup-unpriv`
  `0.5.0`.
- Test support migration:
  shared helper and template provisioning still produce isolated temporary
  databases for each test.
- Environment contract:
  unsupported `PG_TEST_BACKEND` values are treated as explicit skips/failures
  per harness policy; supported values continue working.
- Documentation:
  `docs/developers-guide.md` accurately describes the embedded Postgres test
  strategy and v0.5.0 usage.
- Quality gates:
  `make check-fmt`, `make lint`, and `make test` all pass.

## Idempotence and Recovery

- Most steps are idempotent and safe to re-run.
- If dependency resolution or compilation fails midway, fix the issue and rerun
  from the failed stage.
- If temporary test artefacts accumulate, remove workspace cache directories
  under `target/pg-embed/` and rerun tests.
- If full test runs fail with transient bootstrap network errors, rerun once
  after confirming unchanged code, then treat repeated failure as a tolerance
  breach.

## Artifacts and Notes

Implementation should preserve concise evidence:

- `git diff -- backend/Cargo.toml backend/tests/support/pg_embed.rs backend/tests/support/embedded_postgres.rs docs/developers-guide.md .github/workflows/ci.yml`
- `/tmp/check-fmt-${PROJECT}-${BRANCH}.out`
- `/tmp/lint-${PROJECT}-${BRANCH}.out`
- `/tmp/test-${PROJECT}-${BRANCH}.out`

## Interfaces and Dependencies

The migration must keep using these interfaces and dependencies:

- Crate dependency:
  `pg_embedded_setup_unpriv = { package = "pg-embed-setup-unpriv", version = "0.5.0" }`
  in `backend/Cargo.toml`.
- Test helper contract:
  `backend/tests/support/pg_embed.rs` continues to provide a shared cluster
  accessor suitable for existing test suites.
- Template provisioning contract:
  `backend/tests/support/embedded_postgres.rs::provision_template_database`
  remains the single entry point for per-test temporary databases.
- Skip policy:
  `backend/tests/support/cluster_skip.rs::handle_cluster_setup_failure`
  remains the gatekeeper for optional test skipping.

## Revision Note

Updated after implementation approval to record dependency migration to `0.5.0`,
the `ClusterHandle` split-bootstrap adoption, strict `PG_TEST_BACKEND`
alignment, and final quality-gate evidence. Remaining work has been completed.
