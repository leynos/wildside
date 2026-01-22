# Adopt pg-embed-setup-unpriv 0.2.0

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETED

No PLANS.md found in this repository.

## Purpose / Big Picture

Adopt pg-embed-setup-unpriv v0.2.0 for backend integration tests and take
advantage of the new template-database and persistent-session workflows to
reduce test overhead while keeping behaviour identical. Success is visible
when the backend integration tests still pass, and per-test databases are
created from a template (fast clone) instead of re-running migrations for each
case. The new helper flow should be observable by running the test suite and
seeing no functional differences, only faster setup.

## Constraints

- Only change test support and documentation unless explicitly approved.
- Keep existing test behaviour and failure modes intact.
- Use caret version requirements in Cargo manifests (no wildcards).
- Run `make fmt`, `make lint`, `make test`, and `make check-fmt` before each
  commit, capturing output with `tee`.
- Do not disable or soften test assertions, even for speed.
- Maintain en-GB spelling in documentation and comments.

## Tolerances (Exception Triggers)

- Scope: if adoption requires changes to more than 8 files or 350 net lines,
  stop and escalate.
- Interface: if a public API signature in backend code must change, stop and
  escalate.
- Dependencies: if a new dependency or feature flag is required, stop and
  escalate.
- Tests: if `make test` fails after two attempts, stop and escalate.
- Time: if any stage exceeds 2 hours, stop and escalate.
- Ambiguity: if two plausible isolation strategies exist and either materially
  affects behaviour, stop and request direction.

## Risks

- Risk: Shared cluster reuse could leak data between tests.
  Severity: medium
  Likelihood: medium
  Mitigation: Use template clones or temporary databases per test with
  unique names and Resource Acquisition Is Initialization (RAII) cleanup;
  keep cluster-level isolation tests on the
  per-test cluster path.

- Risk: Template creation could race under parallel tests.
  Severity: low
  Likelihood: medium
  Mitigation: Use `ensure_template_exists` with its built-in locking and
  keep template creation centralized in one helper.

- Risk: v0.2.0 changes default behaviour (paths, env handling) causing
  unexpected failures in CI.
  Severity: medium
  Likelihood: low
  Mitigation: Keep the existing environment scoping in
  `backend/tests/support/pg_embed.rs` and validate with full test runs.

## Progress

- [x] (2026-01-12 00:00Z) Drafted ExecPlan from updated user guide.
- [x] (2026-01-12 00:25Z) Received approval; begin implementation.
- [x] (2026-01-12 00:27Z) Updated user guide for v0.2.0 note.
- [x] (2026-01-12 00:40Z) Commit docs updates before code changes.
- [x] (2026-01-12 01:05Z) Update Cargo dependency to v0.2.0 and refresh lockfile.
- [x] (2026-01-12 01:25Z) Add shared cluster and template database helpers for tests.
- [x] (2026-01-12 01:35Z) Migrate integration tests to template-backed
  databases where safe.
- [x] (2026-01-12 01:55Z) Update documentation that references the old
  per-test bootstrap.
- [x] (2026-01-12 03:05Z) Group embedded Postgres tests and extend their
  nextest timeout to reduce flaky slow-suite timeouts.
- [x] (2026-01-12 03:40Z) Swap cmd-mox source to PyPI release for
  scripts-test stability.
- [x] (2026-01-12 10:30Z) Run quality gates and commit.

## Surprises & Discoveries

- Observation: Embedded Postgres suites intermittently exceeded the 60s
  nextest slow-timeout when run serially.
  Evidence: `make test` timed out on repository integration tests.
  Impact: Added a 120s timeout override for pg-embedded test binaries.

- Observation: The git-pinned cmd-mox helper failed under pytest with a
  `platform.system` AttributeError from generated stubs.
  Evidence: `make scripts-test` failed in vault bootstrap tests.
  Impact: Switched to the latest PyPI cmd-mox release.

- Observation: One `make test` run failed with a transient embedded
  Postgres setup timeout (`deadline has elapsed`).
  Evidence: `backend::ports_behaviour` setup failed before tests ran.
  Impact: Re-ran `make test`, which completed successfully.

## Decision Log

- Decision: Prefer shared cluster plus template clones for tests that only
  need database-level isolation; keep per-test clusters for tests that need
  cluster-level changes.
  Rationale: This balances speed improvements with predictable isolation.
  Date/Author: 2026-01-12 (assistant).

- Decision: Commit documentation updates before code changes.
  Rationale: Requested by the user before implementation proceeds.
  Date/Author: 2026-01-12 (assistant).

- Decision: Pause for approval after exceeding the file-change tolerance.
  Rationale: Changes now span 10 files, above the plan's 8-file limit.
  Date/Author: 2026-01-12 (assistant).

- Decision: Increase the slow-timeout for pg-embedded test binaries to
  120s in nextest.
  Rationale: Serialized integration suites can exceed 60s without failure.
  Date/Author: 2026-01-12 (assistant).

- Decision: Use cmd-mox from PyPI (v0.2.0) instead of a git SHA.
  Rationale: PyPI release avoids stub failures seen in scripts-test.
  Date/Author: 2026-01-12 (assistant).

## Outcomes & Retrospective

- Outcome: Adopted pg-embed-setup-unpriv v0.2.0 with shared cluster and
  template-backed databases for integration tests, preserving behaviour
  while reducing migration overhead.
- Outcome: Stabilized embedded Postgres suites with a longer nextest
  timeout override and switched scripts-test to the PyPI cmd-mox release.
- Validation: `make fmt`, `make lint`, `make test`, `make check-fmt`
  (logs in `tmp/pg-embed-setup-unpriv-0-2-0-*.log`).
- Follow-up: Monitor CI runtimes and flaky rates for pg-embedded suites; if
  needed, revisit template initialization or timeout thresholds.

## Context and Orientation

The backend integration tests rely on `pg_embedded_setup_unpriv::TestCluster`
from the `pg-embed-setup-unpriv` crate. The dependency is declared in
`backend/Cargo.toml`. Test bootstrapping and environment scoping live in
`backend/tests/support/pg_embed.rs`, while database reset and migrations live
in `backend/tests/support/embedded_postgres.rs`. Integration suites such as
`backend/tests/diesel_user_repository.rs`,
`backend/tests/diesel_user_preferences_repository.rs`,
`backend/tests/diesel_route_annotation_repository.rs`, and
`backend/tests/ports_behaviour.rs` all call the test cluster helper.

A template database is a PostgreSQL database used as a clone source. Creating
new databases from a template copies data files directly, which is far faster
than applying migrations each time. Persistent sessions refer to reusing a
single running PostgreSQL cluster across tests (a shared cluster) instead of
starting a fresh cluster per test.

The updated user guide at
`docs/pg-embed-setup-unpriv-users-guide.md` describes
API helpers such as `shared_cluster`, `shared_test_cluster`,
`create_database_from_template`, `ensure_template_exists`, and
`temporary_database_from_template`. This plan uses those APIs as the new
baseline for test setup.

## Plan of Work

Stage A: Confirm the current test usage and baseline.
Review the helper modules and note where per-test clusters are required versus
where database-level isolation is enough. Run a targeted integration test to
confirm the current behaviour before changing dependencies. Do not change code
in this stage.

Stage B: Update the dependency to v0.2.0 and fix compile errors.
Update `backend/Cargo.toml` to `pg_embedded_setup_unpriv = { package =
"pg-embed-setup-unpriv", version = "0.2.0" }`, then refresh `Cargo.lock` with
`cargo update -p`. Address any API changes revealed by `cargo check` or the
next test run.

Stage C: Introduce shared cluster and template-backed database helpers.
Extend `backend/tests/support/pg_embed.rs` with a shared cluster wrapper that
maintains the existing environment scoping but calls `shared_cluster()` or the
`shared_test_cluster` fixture instead of `TestCluster::new()` where safe. Add
helpers in `backend/tests/support/embedded_postgres.rs` to create or reuse a
migration template via `ensure_template_exists`, and return a per-test
`TemporaryDatabase` using `temporary_database_from_template`. Update test
setup code to call the new helper and use the per-test database URL it
provides. Keep a path for tests that require full cluster isolation to
continue using `test_cluster()`.

Stage D: Documentation and cleanup.
Update any documentation that references the old per-test bootstrap flow to
include the template workflow and shared cluster guidance. Validate all
quality gates and commit.

Each stage ends with validation. Do not proceed to the next stage until the
current stage succeeds.

## Concrete Steps

Run commands from the repository root.

1. Inspect current usage and baseline:

   ```bash
   rg "pg_embedded_setup_unpriv" backend/tests
   ```

2. Update the dependency and lockfile:

   ```bash
   rg "pg-embed-setup-unpriv" backend/Cargo.toml
   # edit backend/Cargo.toml to version 0.2.0
   cargo update -p pg-embed-setup-unpriv --precise 0.2.0
   ```

3. Implement shared cluster and template helpers:

   ```bash
   # edit backend/tests/support/pg_embed.rs
   # edit backend/tests/support/embedded_postgres.rs
   # update affected test files to use the new helpers
   ```

4. Update docs that mention test bootstrap strategy:

   ```bash
   rg "pg-embed-setup-unpriv" docs
   # edit relevant docs files to reference template workflow
   ```

5. Run formatting, lint, tests, and format check with logs:

   ```bash
   make fmt | tee /tmp/pg-embed-setup-unpriv-0-2-0-fmt.log
   make lint | tee /tmp/pg-embed-setup-unpriv-0-2-0-lint.log
   make test | tee /tmp/pg-embed-setup-unpriv-0-2-0-test.log
   make check-fmt | tee /tmp/pg-embed-setup-unpriv-0-2-0-check-fmt.log
   ```

6. Review changes and commit:

   ```bash
   git status -sb
   git add backend/Cargo.toml Cargo.lock backend/tests/support \
       backend/tests docs
   git commit -m "Adopt pg-embed-setup-unpriv 0.2.0" \
       -m "Upgrade pg-embed-setup-unpriv to v0.2.0 and update integration test bootstrap."
   ```

Expected signals:

- `cargo update` reports pg-embed-setup-unpriv at 0.2.0 in `Cargo.lock`.
- `make test` exits with status 0 and shows integration tests passing.

## Validation and Acceptance

Acceptance means:

- Integration tests still pass with the updated dependency.
- Per-test databases are created from a template (fast clone) rather than
  re-running migrations each time.
- Shared cluster usage does not leak data across tests (each test sees a clean
  database).
- Documentation describes the new template and shared-cluster flows.

Quality criteria:

- Tests: `make test` passes.
- Lint/typecheck: `make lint` passes with no warnings.
- Formatting: `make fmt` and `make check-fmt` pass.

## Idempotence and Recovery

All steps are re-runnable. If a template database or shared cluster becomes
corrupted, delete the test PostgreSQL directories under the workspace-backed
path (for example, the `target/pg-embed` subtree created by the helper) and
re-run the bootstrap. Recreate the template by re-running the first test that
needs it.

## Artifacts and Notes

Keep short logs showing the successful update and test run in the files
created by `tee` in `/tmp`. If unexpected failures occur, capture the error
output and add it to `Surprises & Discoveries`.

## Interfaces and Dependencies

This change must result in:

- `backend/Cargo.toml` depending on `pg-embed-setup-unpriv` version `0.2.0`.
- `backend/tests/support/pg_embed.rs` providing a shared cluster helper that
  uses `pg_embedded_setup_unpriv::test_support::shared_cluster` or the
  `shared_test_cluster` fixture, while preserving environment scoping.
- `backend/tests/support/embedded_postgres.rs` exposing a helper that calls
  `TestCluster::ensure_template_exists` and returns a per-test
  `TemporaryDatabase` via `temporary_database_from_template`.
- Tests that only need database-level isolation updated to use the template
  helper, with unique database names to avoid collisions.

## Revision note

Initial draft created on 2026-01-12 based on the updated user guide.
Revision on 2026-01-12: Status set to IN PROGRESS, progress updated to
record approval and the user-guide update, and the decision log now
captures the doc-first commit requirement.
Revision on 2026-01-12: Concrete Steps command blocks indented to satisfy
markdownlint, and the docs-commit progress item timestamped.
Revision on 2026-01-12: Progress updated for dependency and test changes,
and the Decision Log now records the file-count tolerance escalation.
Revision on 2026-01-12: Updated the testing guide to describe the shared
cluster and template database strategy.
