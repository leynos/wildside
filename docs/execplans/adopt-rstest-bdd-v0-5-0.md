# Migrate behavioural tests to rstest-bdd v0.5.0

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` are maintained as work proceeds.

Status: COMPLETE

No `PLANS.md` file exists in this repository root.

## Purpose / Big picture

Upgrade repository behavioural suites from `rstest-bdd` `0.4.0` to `0.5.0`,
improve compile-time safety using strict validation, and reduce fixture-noise
boilerplate where safe. Keep all existing behavioural coverage intact and keep
quality gates green.

Observable success:

- `make check-fmt`, `make lint`, and `make test` succeed.
- `backend` and `crates/example-data` use `rstest-bdd` and macros `0.5.0`.
- Behavioural tests compile and run without legacy assumptions that async steps
  are unsupported.
- Contributor-facing strategy is documented in `docs/developers-guide.md`.

## Constraints

- Do not change production runtime behaviour while migrating tests.
- Keep Gherkin scenario intent unchanged.
- Preserve existing gateway commands and pass them before commit.
- Keep dependency specifiers caret-compatible.
- Avoid file-wide lint suppressions for scenario glue.

## Tolerances (exception triggers)

- If migration requires new third-party crates, stop and escalate.
- If behavioural intent cannot be preserved for a feature scenario, stop and
  escalate.
- If full gates fail after three focused repair iterations, stop and escalate.
- If scope exceeds 45 edited tracked files, stop and escalate.

## Risks

- Risk: fixture naming and scenario binding behaviour changed in v0.5.0.
  Severity: medium
  Likelihood: high
  Mitigation: verify fixture-key behaviour with targeted nextest runs before
  full gateway.

- Risk: strict compile-time validation surfaces latent mismatches.
  Severity: medium
  Likelihood: medium
  Mitigation: compile targeted backend and example-data test binaries after
  dependency upgrade.

- Risk: documentation drift around missing developer guide path.
  Severity: medium
  Likelihood: high
  Mitigation: create `docs/developers-guide.md` and cross-link it from testing
  docs.

## Progress

- [x] (2026-02-08 21:12Z) Confirm branch and baseline gates before changes.
- [x] (2026-02-08 21:20Z) Upgrade dependencies to `rstest-bdd` `0.5.0` and
  macros `0.5.0` with strict compile-time validation.
- [x] (2026-02-08 21:29Z) Migrate behavioural test bindings and comments,
  preserving fixture compatibility and passing full gates.
- [x] (2026-02-08 23:04Z) Add and align contributor documentation, rerun full
  gates, and commit docs.

## Surprises & discoveries

- Observation: `docs/developers-guide.md` did not exist, despite references
  from other docs.
  Evidence: file lookup failed during planning; docs contained links to this
  path.
  Impact: migration required creating a new canonical developer guide.

- Observation: underscore-prefixed scenario fixture bindings (`_world`) changed
  runtime fixture keys and broke step injection where steps expected `world`.
  Evidence: nextest failures in backend scenario binaries reported available
  fixture `_world` and missing fixture `world`.
  Impact: scenario bindings in this repository keep `world` key and use explicit
  no-op bodies to satisfy warning gates.

## Decision log

- Decision: enable `strict-compile-time-validation` in macro dev-dependencies.
  Rationale: stronger compile-time drift detection.
  Date/Author: 2026-02-08 / user + Codex

- Decision: retain `world` fixture key in scenario bindings and avoid
  underscore-prefixed binding names in current suites.
  Rationale: existing step definitions depend on `world`; `_world` caused
  runtime fixture mismatch.
  Date/Author: 2026-02-08 / Codex

- Decision: create `docs/developers-guide.md` as the canonical contributor
  strategy document and keep `docs/wildside-testing-guide.md` as operational
  quick reference.
  Rationale: explicit user preference and existing broken-reference risk.
  Date/Author: 2026-02-08 / user + Codex

## Outcomes & retrospective

Code migration outcomes:

- Behavioural dependency upgrade and lockfile refresh completed.
- Behavioural suites pass full gates after fixture-key compatibility fixes.
- Async-support guidance comments were updated to match v0.5.0 reality.

Complete:

- Contributor documentation updates are finalized and validated.
- The docs commit was created after successful full-gate validation.

## Context and orientation

Primary files touched by migration:

- Dependency manifests:
  - `backend/Cargo.toml`
  - `crates/example-data/Cargo.toml`
  - `Cargo.lock`
- Backend behavioural suites:
  - `backend/tests/*_bdd.rs`
  - `backend/tests/diesel_user_repository.rs`
  - `backend/tests/diesel_example_data_runs_repository.rs`
  - `backend/tests/ports_behaviour.rs`
- Example-data behavioural suites:
  - `crates/example-data/tests/*_bdd.rs`
- Documentation:
  - `docs/developers-guide.md`
  - `docs/wildside-testing-guide.md`

## Plan of work

Stage A completed: dependency and behavioural suite migration with gate checks.

Stage B in progress: documentation alignment.

Stage C pending completion: rerun full gates after doc updates and commit docs.

## Concrete steps

Commands used for validation:

- `make check-fmt | tee /tmp/check-fmt-$(basename "$PWD")-$(git branch --show).out`
- `make lint | tee /tmp/lint-$(basename "$PWD")-$(git branch --show).out`
- `make test | tee /tmp/test-$(basename "$PWD")-$(git branch --show).out`

## Validation and acceptance

Acceptance criteria:

- behavioural dependencies upgraded and lockfile updated,
- behavioural suites pass unchanged feature intent under full gates,
- `docs/developers-guide.md` documents the current strategy,
- final full gate run succeeds before final commit.

## Idempotence and recovery

- All edits are source-controlled and can be reapplied safely.
- If a migration step fails validation, revert the uncommitted hunk and rerun
  targeted tests before full gateways.

## Artifacts and notes

Current gate logs:

- `/tmp/check-fmt-adopt-rstest-bdd-v0-5-0-adopt-rstest-bdd-v0-5-0.out`
- `/tmp/lint-adopt-rstest-bdd-v0-5-0-adopt-rstest-bdd-v0-5-0.out`
- `/tmp/test-adopt-rstest-bdd-v0-5-0-adopt-rstest-bdd-v0-5-0.out`

## Interfaces and dependencies

Final dependency intent:

- `rstest-bdd = "0.5.0"`
- `rstest-bdd-macros = { version = "0.5.0",`
  `features = ["strict-compile-time-validation"] }`

No additional dependency introductions are planned.

## Revision note

2026-02-08: Marked plan status `COMPLETE` after final documentation commit and
successful rerun of `make check-fmt`, `make lint`, and `make test`.
